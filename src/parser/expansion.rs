use crate::PErr;
use std::collections::HashMap;
use crate::types::{Value, ExpandedBuiltIn, BuiltIn, Symbol, Expr, VarId, UnrolledExpr};
use crate::parser::{fold_results, ParseErr, Defn};

/// A list of a function's parameters and its body.
type FnInfo = (Vec<Symbol>, Expr);

/// Evaluate a Mil [Expr], tracking symbols and unrolling fns.
pub trait Evaluator {
    //fn eval(UnrolledExpr) -> MelExpr;
    /// Recursively unroll fn invocations in an [Expr] so that only [BuiltIn]s are left.
    fn expand_fns(&self, e: &Expr) -> Result<UnrolledExpr, ParseErr>;
    fn new(fns: Vec<Defn>) -> Self;
}

pub struct Env {
    // Mapping variables to the location they point to on the heap.
    /// Mapping parameters as defined in a fn definition, to their mangled form.
    mangled: HashMap<Symbol, VarId>,
    /// Tracking fns. Notice [Defn] bodies are [Expr]s, meaning they can use other fns
    /// (non-builtins).
    fns: HashMap<Symbol, FnInfo>,
}

/// A simple mangler that just returns i+1 for the next variable id.
struct LinearMangler { idx: VarId }

impl LinearMangler {
    fn next(&mut self) -> VarId {
        self.idx = self.idx + 1;
        self.idx
    }
}

impl Evaluator for Env {
    fn new(fns: Vec<Defn>) -> Self {
        // Store fns in a hashmap
        let fns: HashMap<Symbol, FnInfo> = fns.into_iter().collect();

        Env {
            mangled: HashMap::new(),
            fns,
        }
    }

    /// Mangle variables and substitute all in body.
    /// Prepend set! ops for each variable parameter to the body.
    /// Return the new expression as an [UnrolledExpr].
    fn expand_fns(&self, expr: &Expr) -> Result<UnrolledExpr, ParseErr> {
        // Start from 2 bcs 0 and 1 memory locations are occupied in the VM
        self.expand_mangle_fns(expr, &mut LinearMangler{ idx:2 })
    }
}

impl Env {
    // Convenience abstraction for repetitive code
    fn expand_binop<F>(&self, e1: &Expr, e2: &Expr, op: F, mangler: &mut LinearMangler)
    -> Result<UnrolledExpr, ParseErr>
        where F: Fn(UnrolledExpr, UnrolledExpr) -> ExpandedBuiltIn<UnrolledExpr>
    {
        let e1 = self.expand_mangle_fns(&e1, mangler)?;
        let e2 = self.expand_mangle_fns(&e2, mangler)?;

        Ok(UnrolledExpr::BuiltIn( Box::new(op(e1, e2)) ))
    }

    fn expand_uniop<F>(&self, e: &Expr, op: F, mangler: &mut LinearMangler)
    -> Result<UnrolledExpr, ParseErr>
        where F: Fn(UnrolledExpr) -> ExpandedBuiltIn<UnrolledExpr>
    {
        let e = self.expand_mangle_fns(&e, mangler)?;
        Ok(UnrolledExpr::BuiltIn( Box::new(op(e)) ))
    }

    // Auxillery function to expand and mangle an expression
    fn expand_mangle_fns(&self, expr: &Expr, mangler: &mut LinearMangler) -> Result<UnrolledExpr, ParseErr>
    {
        match expr {
            // A variable should already be mangled, find its mangled value
            Expr::Var(x) => {
                let v = try_get_var(x, &self.mangled)?;
                Ok(UnrolledExpr::Var(v))
            },
            // For a builtin op, expand its arguments and cast into an ExpandedBuiltIn
            Expr::BuiltIn(b) => match &**b {
                BuiltIn::Vempty => Ok(UnrolledExpr::BuiltIn(Box::new(ExpandedBuiltIn::<UnrolledExpr>::Vempty))),
                BuiltIn::Hash(e) => self.expand_uniop(e, ExpandedBuiltIn::<UnrolledExpr>::Hash, mangler),
                BuiltIn::Not(e) => self.expand_uniop(e, ExpandedBuiltIn::<UnrolledExpr>::Not, mangler),
                BuiltIn::Vlen(e) => self.expand_uniop(e, ExpandedBuiltIn::<UnrolledExpr>::Vlen, mangler),
                BuiltIn::Add(e1,e2) => self.expand_binop(e1, e2, ExpandedBuiltIn::<UnrolledExpr>::Add, mangler),
                BuiltIn::Sub(e1,e2) => self.expand_binop(e1, e2, ExpandedBuiltIn::<UnrolledExpr>::Sub, mangler),
                BuiltIn::Mul(e1,e2) => self.expand_binop(e1, e2, ExpandedBuiltIn::<UnrolledExpr>::Mul, mangler),
                BuiltIn::Div(e1,e2) => self.expand_binop(e1, e2, ExpandedBuiltIn::<UnrolledExpr>::Div, mangler),
                BuiltIn::Rem(e1,e2) => self.expand_binop(e1, e2, ExpandedBuiltIn::<UnrolledExpr>::Rem, mangler),
                BuiltIn::And(e1,e2) => self.expand_binop(e1, e2, ExpandedBuiltIn::<UnrolledExpr>::And, mangler),
                BuiltIn::Or(e1,e2)  => self.expand_binop(e1, e2, ExpandedBuiltIn::<UnrolledExpr>::Or, mangler),
                BuiltIn::Xor(e1,e2) => self.expand_binop(e1, e2, ExpandedBuiltIn::<UnrolledExpr>::Xor, mangler),
                BuiltIn::Vref(e1,e2) => self.expand_binop(e1, e2, ExpandedBuiltIn::<UnrolledExpr>::Vref, mangler),
                BuiltIn::Vappend(e1,e2) => self.expand_binop(e1, e2, ExpandedBuiltIn::<UnrolledExpr>::Vappend, mangler),
                BuiltIn::Vpush(e1,e2) => self.expand_binop(e1, e2, ExpandedBuiltIn::<UnrolledExpr>::Vpush, mangler),
                /*
                BuiltIn::Store(e) => {
                    let e = self.expand_mangle_fns(&e, mangler)?;
                    Ok(UnrolledExpr::BuiltIn( Box::new(ExpandedBuiltIn::<UnrolledExpr>::Store(e)) ))
                },
                */
                _ => todo!("Not all builtins have been implemented"),
            },
            // A `set!` must operate on a bound variable; find it and also expand the assignment expression
            Expr::Set(s,e) => {
                let var = try_get_var(s, &self.mangled)?;
                let expr = self.expand_mangle_fns(e, mangler)?;
                Ok(UnrolledExpr::Set(var, Box::new(expr)))
            },
            // Expand a fn call to its body, fail if a defn is not found
            Expr::App(f,es) => {
                // Get the fn definition from the env
                let (params, body) = self.fns.get(f)
                    .ok_or(ParseErr(format!("Function {} was called but is not deifned.", f)))?;

                // Check that args length macthes params to fn
                if params.len() != es.len() {
                    return PErr!("Function invocation expected {} arguments, {} were supplied.",
                        params.len(), es.len());
                }

                // Expand arguments before expanding body
                let args = fold_results(es.iter()
                    .map(|e| self.expand_mangle_fns(e, mangler)).collect())?;

                // Mangle parameters of fn
                let mangled_vars: Vec<VarId> = params.iter().map(|_| mangler.next()).collect();
                // Map between mangled and original
                let mangled_map: HashMap<Symbol, VarId>
                    = params.clone().into_iter()
                            .zip(mangled_vars.clone().into_iter())
                            .collect();

                // Create a new env to expand the body and replace variables with the mangled version
                let f_env = Env {
                    // TODO: Make sure mangled_map overrides mangled
                    mangled: self.mangled.clone().into_iter().chain(mangled_map).collect(),
                    fns: self.fns.clone(),
                };

                // lol
                let mangled_body = f_env.expand_mangle_fns(body, mangler)?;

                let bindings = mangled_vars.into_iter()
                                           .zip(args.into_iter())
                                           .collect();

                // Wrap our mangled body in let bindings
                Ok(UnrolledExpr::Let(bindings, vec![mangled_body]))
            },
            // Mangling happens here
            Expr::Let(binds, es) => {
                // Generate mangled names for variables
                let mangled_vars: Vec<VarId> = binds.iter().map(|_| mangler.next()).collect();
                // Expand binding expressions
                let expanded_bind_exprs = fold_results(binds.iter()
                    .map(|(_, expr)| self.expand_mangle_fns(expr, mangler)).collect())?;
                // Zip em together for later
                let mangled_binds = mangled_vars.iter().cloned()
                                                .zip(expanded_bind_exprs.iter().cloned())
                                                .collect();

                // Map between mangled and original variable names
                let mangled_map: HashMap<Symbol, VarId>
                    = binds.into_iter()
                           .map(|(s,_)| s.clone())
                           .zip(mangled_vars.into_iter().clone())
                           .collect();

                // Create a new env to expand the body and replace variables with the mangled version
                let f_env = Env {
                    // TODO: Make sure mangled_map overrides mangled
                    mangled: self.mangled.clone().into_iter().chain(mangled_map).collect(),
                    fns: self.fns.clone(),
                };

                // Expand body expressions
                let expanded_es = fold_results(es.iter()
                    .map(|e| f_env.expand_mangle_fns(e, mangler))
                    .collect())?;

                Ok(UnrolledExpr::Let(mangled_binds, expanded_es))
            },
            Expr::Value(v) => match v {
                Value::Int(n) => Ok(UnrolledExpr::Value(Value::Int(n.clone()))),
                Value::Bytes(b) => Ok(UnrolledExpr::Value(Value::Bytes(b.clone()))),
            },
        }
    }
}

fn try_get_var(sym: &Symbol, hm: &HashMap<Symbol, VarId>) -> Result<VarId, ParseErr> {
    hm.get(sym)
        .ok_or(ParseErr(format!("Variable {} is not defined.", sym)))
        .map(|v| v.clone())
}
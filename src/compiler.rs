use crate::types::{ExpandedBuiltIn, HeapPos, MelExpr, PushB, PushI, Value};
use ethnum::U256;
use std::fmt;

#[derive(Clone)]
pub struct BinCode(pub Vec<u8>);

impl fmt::Display for BinCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .fold(String::from(""), |acc, bit| acc + &format!("{:02x?}", bit))
        )
    }
}

pub trait Compile {
    /// Produce MelVM interpretable binary from a data type. Consumes a binary struct
    /// and mutates for efficient allocation.
    fn compile_onto(&self, b: BinCode) -> BinCode;

    // Map on the output of the
    // fn map(&self, other: impl Fn(BinCode) -> BinCode) -> BinCode
}

impl<T: Compile> Compile for ExpandedBuiltIn<T> {
    fn compile_onto(&self, b: BinCode) -> BinCode {
        match self {
            ExpandedBuiltIn::Add(e1, e2) => compile_op(b, 0x10, vec![e1, e2]),
            ExpandedBuiltIn::Sub(e1, e2) => compile_op(b, 0x11, vec![e1, e2]),
            ExpandedBuiltIn::Mul(e1, e2) => compile_op(b, 0x12, vec![e1, e2]),
            ExpandedBuiltIn::Div(e1, e2) => compile_op(b, 0x13, vec![e1, e2]),
            ExpandedBuiltIn::Rem(e1, e2) => compile_op(b, 0x14, vec![e1, e2]),

            ExpandedBuiltIn::And(e1, e2) => compile_op(b, 0x20, vec![e1, e2]),
            ExpandedBuiltIn::Or(e1, e2) => compile_op(b, 0x21, vec![e1, e2]),
            ExpandedBuiltIn::Xor(e1, e2) => compile_op(b, 0x22, vec![e1, e2]),
            ExpandedBuiltIn::Not(e) => compile_op(b, 0x23, vec![e]),
            ExpandedBuiltIn::Eql(e1, e2) => compile_op(b, 0x24, vec![e1, e2]),
            ExpandedBuiltIn::Lt(e1, e2) => compile_op(b, 0x25, vec![e1, e2]),
            ExpandedBuiltIn::Gt(e1, e2) => compile_op(b, 0x26, vec![e1, e2]),
            ExpandedBuiltIn::Shl(e1, e2) => compile_op(b, 0x27, vec![e1, e2]),
            ExpandedBuiltIn::Shr(e1, e2) => compile_op(b, 0x28, vec![e1, e2]),

            ExpandedBuiltIn::ItoB(e) => compile_op(b, 0xc0, vec![e]),
            ExpandedBuiltIn::BtoI(e) => compile_op(b, 0xc1, vec![e]),
            ExpandedBuiltIn::TypeQ(e) => compile_op(b, 0xc2, vec![e]),

            ExpandedBuiltIn::Dup(e) => compile_op(b, 0xff, vec![e]),

            ExpandedBuiltIn::Vref(e1, e2) => compile_op(b, 0x50, vec![e1, e2]),
            ExpandedBuiltIn::Vappend(e1, e2) => compile_op(b, 0x51, vec![e1, e2]),
            ExpandedBuiltIn::Vempty => compile_op::<MelExpr>(b, 0x52, vec![]),
            ExpandedBuiltIn::Vlen(e) => compile_op(b, 0x53, vec![e]),
            ExpandedBuiltIn::Vslice(e1, e2, e3) => compile_op(b, 0x54, vec![e1, e2, e3]),
            ExpandedBuiltIn::Vset(e1, e2, e3) => compile_op(b, 0x55, vec![e1, e2, e3]),
            ExpandedBuiltIn::Vpush(e1, e2) => compile_op(b, 0x56, vec![e1, e2]),
            ExpandedBuiltIn::Vcons(e1, e2) => compile_op(b, 0x57, vec![e1, e2]),

            ExpandedBuiltIn::Bref(e1, e2) => compile_op(b, 0x70, vec![e1, e2]),
            ExpandedBuiltIn::Bappend(e1, e2) => compile_op(b, 0x71, vec![e1, e2]),
            ExpandedBuiltIn::Bempty => compile_op::<MelExpr>(b, 0x72, vec![]),
            ExpandedBuiltIn::Blen(e) => compile_op(b, 0x73, vec![e]),
            ExpandedBuiltIn::Bslice(e1, e2, e3) => compile_op(b, 0x74, vec![e1, e2, e3]),
            ExpandedBuiltIn::Bset(e1, e2, e3) => compile_op(b, 0x75, vec![e1, e2, e3]),
            ExpandedBuiltIn::Bpush(e1, e2) => compile_op(b, 0x76, vec![e1, e2]),
            ExpandedBuiltIn::Bcons(e1, e2) => compile_op(b, 0x77, vec![e1, e2]),

            ExpandedBuiltIn::Jmp(n) => compile_u16op(b, 0xa0, n),
            ExpandedBuiltIn::Bez(n) => compile_u16op(b, 0xa1, n),
            ExpandedBuiltIn::Bnz(n) => compile_u16op(b, 0xa2, n),
            ExpandedBuiltIn::Store(idx) => compile_u16op(b, 0x43, idx),
            ExpandedBuiltIn::Load(idx) => compile_u16op(b, 0x42, idx),
        }
    }
}

fn compile_u16_expr_op<T: Compile>(b: BinCode, opcode: u8, n: &u16, arg: &T) -> BinCode {
    let mut b = arg.compile_onto(b);
    b.0.push(opcode);
    n.compile_onto(b)
}

fn compile_u16op(mut b: BinCode, opcode: u8, idx: &HeapPos) -> BinCode {
    //let mut b = idx.compile_onto(b);
    b.0.push(opcode);
    idx.compile_onto(b)
}

// Compile the args, then append the op (postfix)
fn compile_op<T: Compile>(b: BinCode, opcode: u8, args: Vec<&T>) -> BinCode {
    let mut b = args
        .iter()
        .rev()
        .fold(b, |b_acc, arg| arg.compile_onto(b_acc));
    b.0.push(opcode);
    b
}

// Convenience fn for allocating and appending an op code and number to bincode
fn write_pushi(mut b: BinCode, n: &U256) -> BinCode {
    // Write op + n to bincode
    b.0.push(PushI.into());

    //let idx = b.0.len();

    // Extend vec by 32 bytes to effeciently add U256
    //let b_size = 32;
    //b.0.reserve(b_size);
    //unsafe { b.0.set_len(idx + b_size); }

    b.0.extend(&n.to_be_bytes()); //&mut b.0[idx..]);

    b
}

fn write_pushb(mut b: BinCode, bytes: &[u8]) -> BinCode {
    // Op
    b.0.push(PushB.into());
    // Length of bytestring
    b.0.push(bytes.len() as u8);
    // Bytes
    b.0.extend(bytes.iter());
    b
}

/// Compile a loop expression onto a bincode.
fn write_loop(mut b: BinCode, n: &u16, e: &MelExpr) -> BinCode {
    b.0.push(0xb0);
    let op_cnt: u16 = crate::parser::count_insts(e);
    let b = n.compile_onto(b);
    let b = op_cnt.compile_onto(b);
    e.compile_onto(b)
}

/*
fn write_vec(mut bin: BinCode, v: &Vec<Atom>) -> BinCode {
    // Write v.len() vpush opcodes
    let vpush: u8 = (&BuiltIn::Vpush).into();
    bin.0.extend( (1..v.len()).map(|_| vpush) );

    // Followed by vempty
    bin.0.push( (&BuiltIn::Vempty).into() );

    // Then the elements in reverse
    v.iter().rev().fold(bin, |acc, e| e.compile_onto(acc))
}
*/

impl Compile for MelExpr {
    fn compile_onto(&self, b: BinCode) -> BinCode {
        //println!("writing {:?} to {}", self, b);
        match self {
            MelExpr::Hash(n, e) => compile_u16_expr_op(b, 0x30, n, &**e),
            MelExpr::Sigeok(n, e1, e2, e3) => {
                let mut b = e3.compile_onto(e2.compile_onto(e1.compile_onto(b)));
                b.0.push(0x32);
                n.compile_onto(b)
            }
            MelExpr::Loop(n, e) => write_loop(b, n, e),
            // Integers evaluate to themselves (push onto stack)
            MelExpr::Value(v) => match v {
                Value::Int(n) => write_pushi(b, n),
                Value::Bytes(bytes) => write_pushb(b, bytes),
            },
            // Compile each expression in sequence
            MelExpr::Seq(l) => l.iter().fold(b, |b_acc, expr| expr.compile_onto(b_acc)),
            // Compile the op wth args in postfix
            MelExpr::BuiltIn(op) => op.compile_onto(b),
            MelExpr::Noop => {
                let mut b = b;
                b.0.push(0x09);
                b
            }
        }
    }
}

impl Compile for HeapPos {
    fn compile_onto(&self, mut b: BinCode) -> BinCode {
        b.0.extend_from_slice(&self.to_be_bytes());
        b
    }
}

/// Opcode mapping
impl From<PushI> for u8 {
    fn from(_: PushI) -> u8 {
        0xf1
    }
}
impl From<PushB> for u8 {
    fn from(_: PushB) -> u8 {
        0xf0
    }
}

#[cfg(test)]
mod test {
    /*
    fn compile(code: &str) -> Result<BinCode, ()> {
        // Parse
        let (_, ast) = parser::expr(&code[..])
            .map_err(|_| ())?;

        let empty = BinCode(Vec::new());
        Ok( ast.compile_onto(empty) )
    }

    /// Compile a str of code and disassemble back into ops using melVM.
    fn to_ops(code: &str) -> Result<Vec<OpCode>, ()> {
        let bin = compile(code)?;
        Covenant(bin.0).to_ops().ok_or(())
    }
    */

    // fn assert_veq<T: PartialEq + std::fmt::Debug>(v1: Vec<T>, v2: Vec<T>) {
    //     v1.iter().zip(v2.iter()).for_each(|(x, y)| assert_eq!(x, y))
    // }

    /*
    #[test]
    fn cons_a_vec() {
        let bin = compile("(cons 1 (nil))")
            .unwrap();

        assert_eq!(
            format!("{}", bin),
            "f100000000000000000000000000000000000000000000000000000000000000015254");
    }
    */

    /*
    #[test]
    fn vec_len() {
        let ops = to_ops("(len (cons 2 (cons 1 (nil))))").unwrap();
        let target = vec![PUSHI(U256::from(2)), PUSHI(U256::from(1)), VEMPTY, VPUSH, VPUSH, VLENGTH];

        assert_veq(ops, target);
    }

    #[test]
    fn vec_append() {
        let ops = to_ops("(concat (cons 2 (cons 1 (nil))) (cons 4 (cons 3 (nil))))").unwrap();
        let target = vec![PUSHI(U256::from(2)), PUSHI(U256::from(1)), VEMPTY, VPUSH, VPUSH,
                          PUSHI(U256::from(4)), PUSHI(U256::from(3)), VEMPTY, VPUSH, VPUSH, VLENGTH];

        assert_veq(ops, target);
    }
    */
}

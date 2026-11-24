//! Scalar operations used by the CIL evaluation stack.

pub enum ArOp {
    Add,
    Sub,
    Mul,
    Div,
    DivUn,
    Rem,
    RemUn,
    And,
    Or,
    Xor,
}

#[inline]
pub fn arith_i32(x: i32, y: i32, op: ArOp) -> i32 {
    match op {
        ArOp::Add => x.wrapping_add(y),
        ArOp::Sub => x.wrapping_sub(y),
        ArOp::Mul => x.wrapping_mul(y),
        ArOp::Div => x.wrapping_div(y),
        ArOp::DivUn => ((x as u32) / (y as u32)) as i32,
        ArOp::Rem => x.wrapping_rem(y),
        ArOp::RemUn => ((x as u32) % (y as u32)) as i32,
        ArOp::And => x & y,
        ArOp::Or => x | y,
        ArOp::Xor => x ^ y,
    }
}

#[inline]
pub fn arith_i64(x: i64, y: i64, op: ArOp) -> i64 {
    match op {
        ArOp::Add => x.wrapping_add(y),
        ArOp::Sub => x.wrapping_sub(y),
        ArOp::Mul => x.wrapping_mul(y),
        ArOp::Div => x.wrapping_div(y),
        ArOp::DivUn => ((x as u64) / (y as u64)) as i64,
        ArOp::Rem => x.wrapping_rem(y),
        ArOp::RemUn => ((x as u64) % (y as u64)) as i64,
        ArOp::And => x & y,
        ArOp::Or => x | y,
        ArOp::Xor => x ^ y,
    }
}

#[inline]
pub fn arith_f32(x: f32, y: f32, op: ArOp) -> f32 {
    match op {
        ArOp::Add => x + y,
        ArOp::Sub => x - y,
        ArOp::Mul => x * y,
        ArOp::Div => x / y,
        ArOp::Rem => x % y,
        _ => 0.0,
    }
}

#[inline]
pub fn arith_f64(x: f64, y: f64, op: ArOp) -> f64 {
    match op {
        ArOp::Add => x + y,
        ArOp::Sub => x - y,
        ArOp::Mul => x * y,
        ArOp::Div => x / y,
        ArOp::Rem => x % y,
        _ => 0.0,
    }
}

/// The execution loop owns the instruction pointer and unwinding state.
pub trait Frame {
    type Value;
    type Pending;

    fn step(
        &mut self,
        pc: &mut usize,
        pending: &mut Vec<Self::Pending>,
    ) -> Result<Option<Self::Value>, Self::Value>;

    fn handle_exception(
        &mut self,
        faulting_instruction: usize,
        exception: Self::Value,
        pending: &mut Vec<Self::Pending>,
    ) -> Result<usize, Self::Value>;
}

#[inline]
pub fn execute<F: Frame>(frame: &mut F) -> Result<F::Value, F::Value> {
    let mut pc = 0;
    let mut pending = Vec::new();
    loop {
        match frame.step(&mut pc, &mut pending) {
            Ok(Some(value)) => return Ok(value),
            Ok(None) => {}
            Err(exception) => {
                pc = frame.handle_exception(pc.saturating_sub(1), exception, &mut pending)?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_and_unsigned_integer_operations_keep_their_bit_patterns() {
        assert_eq!(arith_i32(i32::MAX, 1, ArOp::Add), i32::MIN);
        assert_eq!(arith_i64(-1, 2, ArOp::DivUn), i64::MAX);
        assert_eq!(arith_i32(-3, 2, ArOp::Rem), -1);
    }

    #[test]
    fn floating_operations_keep_their_precision() {
        assert_eq!(arith_f32(0.5, 0.25, ArOp::Add), 0.75);
        assert!(arith_f64(0.0, 0.0, ArOp::Div).is_nan());
    }
    #[test]
    fn exceptions_resume_at_the_handler_or_are_propagated() {
        struct TestFrame(bool);
        impl Frame for TestFrame {
            type Value = i32;
            type Pending = i32;

            fn step(&mut self, pc: &mut usize, pending: &mut Vec<i32>) -> Result<Option<i32>, i32> {
                match *pc {
                    0 => {
                        *pc = 2;
                        Ok(None)
                    }
                    2 => {
                        *pc = 3;
                        Err(42)
                    }
                    4 => Ok(pending.pop()),
                    _ => panic!("unexpected instruction {pc}"),
                }
            }

            fn handle_exception(
                &mut self,
                pc: usize,
                error: i32,
                pending: &mut Vec<i32>,
            ) -> Result<usize, i32> {
                assert_eq!(pc, 2);
                if self.0 {
                    pending.push(error);
                    Ok(4)
                } else {
                    Err(error)
                }
            }
        }
        assert_eq!(execute(&mut TestFrame(true)), Ok(42));
        assert_eq!(execute(&mut TestFrame(false)), Err(42));
    }
}

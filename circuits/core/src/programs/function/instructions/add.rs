// Copyright (C) 2019-2022 Aleo Systems Inc.
// This file is part of the snarkVM library.

// The snarkVM library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkVM library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkVM library. If not, see <https://www.gnu.org/licenses/>.

use crate::{
    function::{parsers::*, registers::Registers},
    helpers::Register,
    instructions::Instruction,
    Literal,
    Opcode,
    Operation,
    Program,
    Value,
};
use snarkvm_circuits_types::environment::{Parser, ParserResult};
use snarkvm_utilities::{FromBytes, ToBytes};

use core::fmt;
use nom::combinator::map;
use std::io::{Read, Result as IoResult, Write};

/// Adds `first` with `second`, storing the outcome in `destination`.
pub struct Add<P: Program> {
    operation: BinaryOperation<P>,
}

impl<P: Program> Add<P> {
    /// Returns the operands of the instruction.
    pub fn operands(&self) -> Vec<Operand<P>> {
        self.operation.operands()
    }

    /// Returns the destination register of the instruction.
    pub fn destination(&self) -> &Register<P> {
        self.operation.destination()
    }
}

impl<P: Program> Opcode for Add<P> {
    /// Returns the opcode as a string.
    #[inline]
    fn opcode() -> &'static str {
        "add"
    }
}

impl<P: Program> Operation<P> for Add<P> {
    /// Evaluates the operation.
    #[inline]
    fn evaluate(&self, registers: &mut Registers<P>) {
        // Load the values for the first and second operands.
        let first = match registers.load(self.operation.first()) {
            Value::Literal(literal) => literal,
            Value::Composite(name, ..) => P::halt(format!("{name} is not a literal")),
        };
        let second = match registers.load(self.operation.second()) {
            Value::Literal(literal) => literal,
            Value::Composite(name, ..) => P::halt(format!("{name} is not a literal")),
        };

        // Perform the operation.
        let result = match (first, second) {
            (Literal::Field(a), Literal::Field(b)) => Literal::Field(a + b),
            (Literal::Group(a), Literal::Group(b)) => Literal::Group(a + b),
            (Literal::I8(a), Literal::I8(b)) => Literal::I8(a + b),
            (Literal::U8(a), Literal::U8(b)) => Literal::U8(a + b),
            _ => P::halt(format!("Invalid '{}' instruction", Self::opcode())),
        };

        registers.assign(self.operation.destination(), result);
    }
}

impl<P: Program> Parser for Add<P> {
    type Environment = P;

    /// Parses a string into an 'add' operation.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        // Parse the operation from the string.
        let (string, operation) = map(BinaryOperation::parse, |operation| Self { operation })(string)?;
        // Return the operation.
        Ok((string, operation))
    }
}

impl<P: Program> fmt::Display for Add<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.operation)
    }
}

// impl<P: Program> FromBytes for Add<P>> {
//     fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
//         Ok(Self { operation: BinaryOperation::read_le(&mut reader)? })
//     }
// }
//
// impl<P: Program> ToBytes for Add<P>> {
//     fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
//         self.operation.write_le(&mut writer)
//     }
// }

#[allow(clippy::from_over_into)]
impl<P: Program> Into<Instruction<P>> for Add<P> {
    /// Converts the operation into an instruction.
    fn into(self) -> Instruction<P> {
        Instruction::Add(self)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::{Input, Register, Stack};
//     use snarkvm_circuits_types::environment::{Circuit, Eject};
//
//     #[test]
//     fn test_add_field() {
//         let first = Literal::<Circuit>::from_str("1field.public");
//         let second = Literal::<Circuit>::from_str("2field.private");
//         let expected = Literal::<Circuit>::from_str("3field.private");
//
//         Input::from_str("input r0 field.public;").assign(first).evaluate(&memory);
//         Input::from_str("input r1 field.private;").assign(second).evaluate(&memory);
//
//         Add::<Stack<Circuit>>::from_str("r2 r0 r1").evaluate(&memory);
//         assert_eq!(expected.eject(), memory.load(&Register::new(2)).eject());
//     }
//
//     #[test]
//     fn test_add_group() {
//         let first = Literal::<Circuit>::from_str("2group.public");
//         let second = Literal::<Circuit>::from_str("0group.private");
//         let expected = Literal::<Circuit>::from_str("2group.private");
//
//         Input::from_str("input r0 group.public;").assign(first).evaluate(&memory);
//         Input::from_str("input r1 group.private;").assign(second).evaluate(&memory);
//
//         Add::<Stack<Circuit>>::from_str("r2 r0 r1").evaluate(&memory);
//         assert_eq!(expected.eject(), memory.load(&Register::new(2)).eject());
//     }
// }

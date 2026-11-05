mod common;

use best::{ResultData, ResultSet, ResultType, ResultValue};
use common::format_result_sets;

fn int(ty: ResultType, name: &str, v: i64) -> ResultData {
    ResultData::new(ty, name, ResultValue::Int(v))
}

fn one(data: ResultData) -> String {
    format_result_sets(&[ResultSet(vec![data])])
}

/// Single-line expectation for one value: header, line, trailing blank line.
fn expect_one(line: &str) -> String {
    format!("DATASET: 0\n{}\n\n", line)
}

#[test]
fn oracle_examples() {
    assert_eq!(
        one(int(ResultType::W, "SAETZE", 1)),
        expect_one("SAETZE: W: 1 0x0001")
    );
    assert_eq!(
        one(int(ResultType::I, "UBATTCURRENT", -1)),
        expect_one("UBATTCURRENT: I: -1 0xFFFF")
    );
    assert_eq!(
        one(int(ResultType::L, "ID_SG_ADR", 18)),
        expect_one("ID_SG_ADR: L: 18 0x00000012")
    );
    assert_eq!(
        one(ResultData::new(
            ResultType::S,
            "ECU",
            ResultValue::Str("JBBF".into())
        )),
        expect_one("ECU: JBBF")
    );
    assert_eq!(
        one(ResultData::new(
            ResultType::Y,
            "_TEL_AUFTRAG",
            ResultValue::Bytes(vec![0x82, 0x12, 0xF1, 0x1A, 0x80])
        )),
        expect_one("_TEL_AUFTRAG: 82 12 F1 1A 80 ")
    );
}

#[test]
fn byte_type() {
    assert_eq!(
        one(int(ResultType::B, "X", 255)),
        expect_one("X: B: 255 0xFF")
    );
    assert_eq!(one(int(ResultType::B, "X", 1)), expect_one("X: B: 1 0x01"));
    // Hex masked to 8 bits, decimal stays signed.
    assert_eq!(
        one(int(ResultType::B, "X", -1)),
        expect_one("X: B: -1 0xFF")
    );
}

#[test]
fn char_type() {
    assert_eq!(
        one(int(ResultType::C, "X", 65)),
        expect_one("X: C: 65 0x41")
    );
    assert_eq!(
        one(int(ResultType::C, "X", -2)),
        expect_one("X: C: -2 0xFE")
    );
}

#[test]
fn word_type() {
    assert_eq!(
        one(int(ResultType::W, "X", 0x1234)),
        expect_one("X: W: 4660 0x1234")
    );
    assert_eq!(
        one(int(ResultType::W, "X", -1)),
        expect_one("X: W: -1 0xFFFF")
    );
}

#[test]
fn signed_int_type() {
    assert_eq!(
        one(int(ResultType::I, "X", -2)),
        expect_one("X: I: -2 0xFFFE")
    );
    assert_eq!(
        one(int(ResultType::I, "X", 300)),
        expect_one("X: I: 300 0x012C")
    );
}

#[test]
fn dword_type() {
    assert_eq!(
        one(int(ResultType::D, "X", 4_000_000_000)),
        expect_one("X: D: 4000000000 0xEE6B2800")
    );
    assert_eq!(
        one(int(ResultType::D, "X", -1)),
        expect_one("X: D: -1 0xFFFFFFFF")
    );
}

#[test]
fn long_type() {
    assert_eq!(
        one(int(ResultType::L, "X", -1)),
        expect_one("X: L: -1 0xFFFFFFFF")
    );
    assert_eq!(
        one(int(ResultType::L, "X", 0x7FFF_FFFF)),
        expect_one("X: L: 2147483647 0x7FFFFFFF")
    );
}

#[test]
fn qword_type_prints_qw_label() {
    assert_eq!(
        one(int(ResultType::Q, "X", 1)),
        expect_one("X: QW: 1 0x0000000000000001")
    );
    assert_eq!(
        one(int(ResultType::Q, "X", -1)),
        expect_one("X: QW: -1 0xFFFFFFFFFFFFFFFF")
    );
}

#[test]
fn signed_qword_type_prints_l_label() {
    // Oracle quirk: LL uses the "L:" label with 64-bit hex.
    assert_eq!(
        one(int(ResultType::LL, "X", -2)),
        expect_one("X: L: -2 0xFFFFFFFFFFFFFFFE")
    );
    assert_eq!(
        one(int(ResultType::LL, "X", 3)),
        expect_one("X: L: 3 0x0000000000000003")
    );
}

#[test]
fn float_type() {
    assert_eq!(
        one(ResultData::new(
            ResultType::R,
            "UBATT",
            ResultValue::Float(12.5)
        )),
        expect_one("UBATT: R: 12.500000")
    );
    assert_eq!(
        one(ResultData::new(
            ResultType::R,
            "X",
            ResultValue::Float(-3.25)
        )),
        expect_one("X: R: -3.250000")
    );
    assert_eq!(
        one(ResultData::new(ResultType::R, "X", ResultValue::Float(0.1))),
        expect_one("X: R: 0.100000")
    );
}

#[test]
fn float_value_ignores_declared_type() {
    // C# checks OpData first: a double always prints as "R:".
    assert_eq!(
        one(ResultData::new(ResultType::I, "X", ResultValue::Float(2.0))),
        expect_one("X: R: 2.000000")
    );
}

#[test]
fn integer_with_non_integer_type_prints_question_mark() {
    assert_eq!(one(int(ResultType::R, "X", 5)), expect_one("X: ?"));
    assert_eq!(one(int(ResultType::S, "X", 5)), expect_one("X: ?"));
    assert_eq!(one(int(ResultType::Y, "X", 5)), expect_one("X: ?"));
}

#[test]
fn string_is_raw_and_empty_bytes_leave_trailing_space() {
    assert_eq!(
        one(ResultData::new(
            ResultType::S,
            "X",
            ResultValue::Str(String::new())
        )),
        expect_one("X: ")
    );
    assert_eq!(
        one(ResultData::new(
            ResultType::Y,
            "X",
            ResultValue::Bytes(vec![])
        )),
        expect_one("X: ")
    );
    assert_eq!(
        one(ResultData::new(
            ResultType::Y,
            "X",
            ResultValue::Bytes(vec![0x0A])
        )),
        expect_one("X: 0A ")
    );
}

#[test]
fn keys_sorted_and_datasets_numbered() {
    let set_a = ResultSet(vec![
        int(ResultType::W, "SAETZE", 1),
        ResultData::new(ResultType::S, "ECU", ResultValue::Str("JBBF".into())),
        int(ResultType::L, "ID_SG_ADR", 18),
    ]);
    let set_b = ResultSet(vec![int(ResultType::B, "A", 2)]);
    let out = format_result_sets(&[set_a, set_b]);
    assert_eq!(
        out,
        "DATASET: 0\n\
         ECU: JBBF\n\
         ID_SG_ADR: L: 18 0x00000012\n\
         SAETZE: W: 1 0x0001\n\
         DATASET: 1\n\
         A: B: 2 0x02\n\
         \n"
    );
}

#[test]
fn underscore_sorts_before_letters() {
    let set = ResultSet(vec![
        ResultData::new(ResultType::S, "ECU", ResultValue::Str("x".into())),
        ResultData::new(ResultType::Y, "_TEL_AUFTRAG", ResultValue::Bytes(vec![1])),
    ]);
    let out = format_result_sets(&[set]);
    assert_eq!(out, "DATASET: 0\n_TEL_AUFTRAG: 01 \nECU: x\n\n");
}

#[test]
fn empty_list_prints_only_blank_line() {
    assert_eq!(format_result_sets(&[]), "\n");
}

#[test]
fn empty_set_prints_header_and_blank_line() {
    assert_eq!(
        format_result_sets(&[ResultSet::default()]),
        "DATASET: 0\n\n"
    );
}

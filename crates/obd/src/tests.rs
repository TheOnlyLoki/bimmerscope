//! Protocol tests against the scripted mock link (simulated time).

use best::ErrorCode;
use best::iface::Interface;
use best::telegram::{calc_checksum_bmw_fast, calc_checksum_xor};

use crate::core::{KwpMode, ObdCore, ThreadCommand, Transmit};
use crate::iface::ObdInterface;
use crate::link::{Parity, SerialLink};
use crate::mock::{Ev, Mock};

fn sum(v: &[u8]) -> Vec<u8> {
    let mut v = v.to_vec();
    v.push(calc_checksum_bmw_fast(&v, v.len()));
    v
}

fn xor(v: &[u8]) -> Vec<u8> {
    let mut v = v.to_vec();
    v.push(calc_checksum_xor(&v, v.len()));
    v
}

fn core(mock: &Mock, pars: &[u32]) -> ObdCore {
    let mut c = ObdCore::new();
    c.link = Some(Box::new(mock.clone()) as Box<dyn SerialLink>);
    c.comm_par = Some(pars.to_vec());
    c.apply_comm_parameter().unwrap();
    c
}

/// BMW-FAST: 115200 baud, timeout 1200, regen 0, tel end 20, NR78 2x / 5000 ms.
const BMW_FAST: &[u32] = &[0x010F, 115200, 1200, 0, 20, 2, 5000, 1];
/// DS2: 9600 baud, timeout 1000, regen 0, tel end 20.
const DS2: &[u32] = &[0x0006, 9600, 0, 0, 0, 1000, 0, 20];
/// KWP2000*: 9600 baud, timeout 1000, regen 0, tel end 20, interbyte 0, NR78 2x / 5000 ms.
const KWP2000S: &[u32] = &[0x010D, 9600, 1000, 0, 20, 0, 2, 5000];

/// KWP2000 BMW: 10400 baud, timeout 1000, tel end 20, NR78 2x / 5000 ms,
/// tester present 2000 ms `81 12 F1 3E`, start comm `81 12 F1 81`.
fn kwp2000_bmw_pars() -> Vec<u32> {
    let mut p = vec![0u32; 34];
    p[0] = 0x010C;
    p[1] = 10400;
    p[2] = 1000;
    p[3] = 0;
    p[4] = 20;
    p[5] = 0;
    p[6] = 2;
    p[7] = 5000;
    p[8] = 2000;
    p[9] = 4;
    p[10..14].copy_from_slice(&[0x81, 0x12, 0xF1, 0x3E]);
    p[21] = 4;
    p[22..26].copy_from_slice(&[0x81, 0x12, 0xF1, 0x81]);
    p[33] = 1;
    p
}

// ---------------------------------------------------------------------------
// CommParameter
// ---------------------------------------------------------------------------

#[test]
fn comm_parameter_bmw_fast_sets_port() {
    let mock = Mock::new();
    let c = core(&mock, BMW_FAST);
    assert_eq!(c.transmit, Some(Transmit::BmwFast));
    assert_eq!(
        (
            c.par_timeout_std,
            c.par_timeout_tel_end,
            c.par_retry_nr78,
            c.par_timeout_nr78
        ),
        (1200, 20, 2, 5000)
    );
    assert!(!c.par_send_set_dtr);
    // echo adapter: DTR on, RTS off
    assert_eq!(
        mock.st().events,
        vec![
            Ev::Config(115200, 8, Parity::None),
            Ev::Dtr(true),
            Ev::Rts(false)
        ]
    );
}

#[test]
fn comm_parameter_ds2_even_parity() {
    let mock = Mock::new();
    let c = core(&mock, DS2);
    assert_eq!(c.transmit, Some(Transmit::Ds2));
    assert_eq!(c.comm_answer_len, [-1, 0]);
    assert!(c.ecu_connected);
    assert_eq!(mock.st().events[0], Ev::Config(9600, 8, Parity::Even));
    assert_eq!(mock.st().events[1], Ev::Dtr(false));
}

#[test]
fn comm_parameter_errors() {
    let mock = Mock::new();
    let mut c = core(&mock, BMW_FAST);
    for (pars, err) in [
        (vec![], ErrorCode::EDIABAS_IFH_0041),
        (vec![0x0006, 9600], ErrorCode::EDIABAS_IFH_0041),
        (
            vec![0x0001, 9600, 0, 0, 0, 0, 0, 0],
            ErrorCode::EDIABAS_IFH_0006,
        ),
        (vec![0x0000, 0, 0, 0, 0x81], ErrorCode::EDIABAS_IFH_0014),
        (vec![0x0815], ErrorCode::EDIABAS_IFH_0014),
    ] {
        c.comm_par = Some(pars);
        assert_eq!(c.apply_comm_parameter(), Err(err));
    }
    let mut p = kwp2000_bmw_pars();
    p[9] = 12;
    c.comm_par = Some(p);
    assert_eq!(c.apply_comm_parameter(), Err(ErrorCode::EDIABAS_IFH_0041));
    c.disabled_concepts = vec![0x010F];
    c.comm_par = Some(BMW_FAST.to_vec());
    assert_eq!(c.apply_comm_parameter(), Err(ErrorCode::EDIABAS_IFH_0014));
}

#[test]
fn ds2_default_timeout() {
    let mock = Mock::new();
    let c = core(&mock, &[0x0006, 9600, 0, 0, 0, 0, 0, 20]);
    assert_eq!(c.par_timeout_std, crate::core::DEFAULT_STD_TIMEOUT);
}

// ---------------------------------------------------------------------------
// BMW-FAST
// ---------------------------------------------------------------------------

#[test]
fn bmw_fast_request_checksum_echo_response() {
    let mock = Mock::new();
    let mut c = core(&mock, BMW_FAST);
    let resp = sum(&[0x84, 0xF1, 0x12, 0x5A, 0x80, 0x00, 0x01]);
    mock.expect(&[0x82, 0x12, 0xF1, 0x1A, 0x80, 0x1F], &[(30, &resp)]);
    let r = c.obd_trans(&[0x82, 0x12, 0xF1, 0x1A, 0x80]).unwrap();
    assert_eq!(r, resp);
    assert!(mock.st().unexpected.is_empty());
    // echo removed, nothing left
    assert!(mock.st().rx.is_empty());
    let log = c.log.lock().unwrap().join("\n");
    assert!(log.contains("Send: 82 12 F1 1A 80 1F"), "{log}");
    assert!(log.contains("Echo: 82 12 F1 1A 80 1F"), "{log}");
}

#[test]
fn bmw_fast_ignores_request_checksum_byte() {
    // the checksum is always calculated; a request with checksum gets it replaced
    let mock = Mock::new();
    let mut c = core(&mock, BMW_FAST);
    let resp = sum(&[0x81, 0xF1, 0x12, 0x7E]);
    mock.expect(&sum(&[0x81, 0x12, 0xF1, 0x3E]), &[(10, &resp)]);
    assert_eq!(c.obd_trans(&[0x81, 0x12, 0xF1, 0x3E, 0x00]).unwrap(), resp);
}

#[test]
fn bmw_fast_length_byte_response() {
    let mock = Mock::new();
    let mut c = core(&mock, BMW_FAST);
    let mut data = vec![0x80, 0xF1, 0x12, 0x45, 0x61];
    data.extend(std::iter::repeat_n(0xAA, 0x44));
    let resp = sum(&data);
    mock.expect(&sum(&[0x82, 0x12, 0xF1, 0x21, 0x01]), &[(20, &resp)]);
    assert_eq!(c.obd_trans(&[0x82, 0x12, 0xF1, 0x21, 0x01]).unwrap(), resp);
}

#[test]
fn bmw_fast_wrong_checksum() {
    let mock = Mock::new();
    let mut c = core(&mock, BMW_FAST);
    let mut resp = sum(&[0x83, 0xF1, 0x12, 0x5A, 0x80, 0x00]);
    *resp.last_mut().unwrap() ^= 0xFF;
    mock.expect(&sum(&[0x82, 0x12, 0xF1, 0x1A, 0x80]), &[(30, &resp)]);
    assert_eq!(
        c.obd_trans(&[0x82, 0x12, 0xF1, 0x1A, 0x80]),
        Err(ErrorCode::EDIABAS_IFH_0009)
    );
}

#[test]
fn bmw_fast_checksum_no_check() {
    let mock = Mock::new();
    let mut c = core(&mock, &[0x010F, 115200, 1200, 0, 20, 2, 5000, 3]);
    let mut resp = sum(&[0x83, 0xF1, 0x12, 0x5A, 0x80, 0x00]);
    *resp.last_mut().unwrap() ^= 0xFF;
    mock.expect(&sum(&[0x82, 0x12, 0xF1, 0x1A, 0x80]), &[(30, &resp)]);
    assert_eq!(c.obd_trans(&[0x82, 0x12, 0xF1, 0x1A, 0x80]).unwrap(), resp);
}

#[test]
fn bmw_fast_timeout() {
    let mock = Mock::new();
    let mut c = core(&mock, BMW_FAST);
    mock.expect(&sum(&[0x82, 0x12, 0xF1, 0x1A, 0x80]), &[]);
    let t0 = mock.now();
    assert_eq!(
        c.obd_trans(&[0x82, 0x12, 0xF1, 0x1A, 0x80]),
        Err(ErrorCode::EDIABAS_IFH_0009)
    );
    let waited = (mock.now() - t0) / 1000;
    // timeout std + AddRecTimeout
    assert!((1220..1240).contains(&waited), "{waited}");
}

#[test]
fn bmw_fast_no_echo() {
    let mock = Mock::new();
    mock.st().echo = false;
    let mut c = core(&mock, BMW_FAST);
    mock.expect(&sum(&[0x82, 0x12, 0xF1, 0x1A, 0x80]), &[]);
    assert_eq!(
        c.obd_trans(&[0x82, 0x12, 0xF1, 0x1A, 0x80]),
        Err(ErrorCode::EDIABAS_IFH_0003)
    );
}

#[test]
fn bmw_fast_wrong_echo() {
    let mock = Mock::new();
    mock.st().corrupt_echo = true;
    let mut c = core(&mock, BMW_FAST);
    c.comm_repeats = 2;
    mock.expect(&sum(&[0x82, 0x12, 0xF1, 0x1A, 0x80]), &[]);
    // IFH_0003 stops the retries
    assert_eq!(
        c.obd_trans(&[0x82, 0x12, 0xF1, 0x1A, 0x80]),
        Err(ErrorCode::EDIABAS_IFH_0003)
    );
    assert_eq!(mock.writes().len(), 1);
}

#[test]
fn bmw_fast_without_echo_adapter() {
    let mock = Mock::new();
    mock.st().echo = false;
    let mut c = ObdCore::new();
    c.has_adapter_echo = false;
    c.link = Some(Box::new(mock.clone()));
    c.comm_par = Some(BMW_FAST.to_vec());
    c.apply_comm_parameter().unwrap();
    assert!(c.par_send_set_dtr);
    let resp = sum(&[0x81, 0xF1, 0x12, 0x7E]);
    mock.expect(&sum(&[0x81, 0x12, 0xF1, 0x3E]), &[(10, &resp)]);
    assert_eq!(c.obd_trans(&[0x81, 0x12, 0xF1, 0x3E]).unwrap(), resp);
    // DTR pulse around the telegram
    let ev = mock.st().events.clone();
    let w = ev.iter().position(|e| matches!(e, Ev::Write(..))).unwrap();
    assert_eq!(ev[w - 1], Ev::Dtr(true));
    assert_eq!(ev[w + 1], Ev::Dtr(false));
}

#[test]
fn bmw_fast_nr78_waits_with_nr78_timeout() {
    let mock = Mock::new();
    let mut c = core(&mock, BMW_FAST);
    let pending = sum(&[0x83, 0xF1, 0x12, 0x7F, 0x31, 0x78]);
    let resp = sum(&[0x82, 0xF1, 0x12, 0x71, 0x01]);
    // final response 3 s after NR78: longer than timeout std (1200), shorter than NR78 timeout (5000)
    mock.expect(
        &sum(&[0x82, 0x12, 0xF1, 0x31, 0x01]),
        &[(50, &pending), (3000, &resp)],
    );
    assert_eq!(c.obd_trans(&[0x82, 0x12, 0xF1, 0x31, 0x01]).unwrap(), resp);
    assert!(c.nr78_dict.is_empty());
}

#[test]
fn bmw_fast_nr78_exceeded_returns_pending() {
    let mock = Mock::new();
    let mut c = core(&mock, BMW_FAST);
    let pending = sum(&[0x83, 0xF1, 0x12, 0x7F, 0x31, 0x78]);
    let p: &[u8] = &pending;
    mock.expect(
        &sum(&[0x82, 0x12, 0xF1, 0x31, 0x01]),
        &[(50, p), (100, p), (100, p), (100, p)],
    );
    // retries 2: 3rd repeat exceeds, the NR78 telegram is the response
    assert_eq!(
        c.obd_trans(&[0x82, 0x12, 0xF1, 0x31, 0x01]).unwrap(),
        pending
    );
}

#[test]
fn bmw_fast_nr78_timeout() {
    let mock = Mock::new();
    let mut c = core(&mock, BMW_FAST);
    let pending = sum(&[0x83, 0xF1, 0x12, 0x7F, 0x31, 0x78]);
    mock.expect(&sum(&[0x82, 0x12, 0xF1, 0x31, 0x01]), &[(50, &pending)]);
    let t0 = mock.now();
    assert_eq!(
        c.obd_trans(&[0x82, 0x12, 0xF1, 0x31, 0x01]),
        Err(ErrorCode::EDIABAS_IFH_0009)
    );
    let waited = (mock.now() - t0) / 1000;
    assert!((5050..5100).contains(&waited), "{waited}");
}

#[test]
fn bmw_fast_retry_after_timeout() {
    let mock = Mock::new();
    let mut c = core(&mock, BMW_FAST);
    c.comm_repeats = 1;
    let req = sum(&[0x82, 0x12, 0xF1, 0x1A, 0x80]);
    let resp = sum(&[0x83, 0xF1, 0x12, 0x5A, 0x80, 0x00]);
    mock.expect(&req, &[]);
    mock.expect(&req, &[(20, &resp)]);
    assert_eq!(c.obd_trans(&[0x82, 0x12, 0xF1, 0x1A, 0x80]).unwrap(), resp);
    assert_eq!(mock.writes().len(), 2);

    // RetryComm = 0 disables the repeats
    c.retry_comm = false;
    mock.expect(&req, &[]);
    mock.expect(&req, &[(20, &resp)]);
    assert_eq!(
        c.obd_trans(&[0x82, 0x12, 0xF1, 0x1A, 0x80]),
        Err(ErrorCode::EDIABAS_IFH_0009)
    );
}

#[test]
fn bmw_fast_receive_only_next_telegram() {
    // functional request: further responses are fetched with an empty request
    let mock = Mock::new();
    let mut c = core(&mock, BMW_FAST);
    let r1 = sum(&[0x82, 0xF1, 0x12, 0x7E, 0x00]);
    let r2 = sum(&[0x82, 0xF1, 0x40, 0x7E, 0x00]);
    mock.expect(
        &sum(&[0xC2, 0xDF, 0xF1, 0x3E, 0x00]),
        &[(10, &r1), (5, &r2)],
    );
    assert_eq!(c.obd_trans(&[0xC2, 0xDF, 0xF1, 0x3E, 0x00]).unwrap(), r1);
    assert_eq!(c.obd_trans(&[]).unwrap(), r2);
    assert_eq!(c.obd_trans(&[]), Err(ErrorCode::EDIABAS_IFH_0009));
}

#[test]
fn not_connected_and_no_parameter() {
    let mut c = ObdCore::new();
    assert_eq!(c.obd_trans(&[1]), Err(ErrorCode::EDIABAS_IFH_0019));
    c.link = Some(Box::new(Mock::new()));
    assert_eq!(c.obd_trans(&[1]), Err(ErrorCode::EDIABAS_IFH_0006));
}

// ---------------------------------------------------------------------------
// DS2
// ---------------------------------------------------------------------------

#[test]
fn ds2_request_checksum_echo_response() {
    let mock = Mock::new();
    let mut c = core(&mock, DS2);
    let resp = xor(&[0x12, 0x05, 0xA0, 0x88]);
    mock.expect(&[0x12, 0x04, 0x00, 0x16], &[(40, &resp)]);
    assert_eq!(c.obd_trans(&[0x12, 0x04, 0x00]).unwrap(), resp);
    assert!(mock.st().unexpected.is_empty());
}

#[test]
fn ds2_wrong_checksum() {
    let mock = Mock::new();
    let mut c = core(&mock, DS2);
    let resp = [0x12, 0x05, 0xA0, 0x88, 0x00];
    mock.expect(&xor(&[0x12, 0x04, 0x00]), &[(40, &resp)]);
    assert_eq!(
        c.obd_trans(&[0x12, 0x04, 0x00]),
        Err(ErrorCode::EDIABAS_IFH_0009)
    );
}

#[test]
fn ds2_timeout_and_short_tail() {
    let mock = Mock::new();
    let mut c = core(&mock, DS2);
    mock.expect(&xor(&[0x12, 0x04, 0x00]), &[]);
    let t0 = mock.now();
    assert_eq!(
        c.obd_trans(&[0x12, 0x04, 0x00]),
        Err(ErrorCode::EDIABAS_IFH_0009)
    );
    assert!((mock.now() - t0) / 1000 >= 1020);
    // length byte says 6, only 5 bytes arrive
    mock.expect(
        &xor(&[0x12, 0x04, 0x00]),
        &[(40, &[0x12, 0x06, 0xA0, 0x88, 0x3C])],
    );
    assert_eq!(
        c.obd_trans(&[0x12, 0x04, 0x00]),
        Err(ErrorCode::EDIABAS_IFH_0009)
    );
}

#[test]
fn ds2_fixed_answer_length() {
    let mock = Mock::new();
    let mut c = core(&mock, DS2);
    c.comm_answer_len = [3, 0];
    let resp = xor(&[0xA0, 0x01]);
    mock.expect(&xor(&[0x12, 0x04, 0x00]), &[(40, &resp)]);
    assert_eq!(c.obd_trans(&[0x12, 0x04, 0x00]).unwrap(), resp);
}

#[test]
fn ds2_interbyte_time() {
    let mock = Mock::new();
    let mut c = core(&mock, &[0x0006, 9600, 0, 0, 0, 1000, 0, 20, 2]);
    let resp = xor(&[0x12, 0x05, 0xA0, 0x88]);
    mock.expect(&xor(&[0x12, 0x04, 0x00]), &[(40, &resp)]);
    assert_eq!(c.obd_trans(&[0x12, 0x04, 0x00]).unwrap(), resp);
    let times: Vec<u64> = mock
        .st()
        .events
        .iter()
        .filter_map(|e| {
            if let Ev::Write(_, t) = e {
                Some(*t)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(times.len(), 4);
    // 2 ms + 11 bit at 9600 baud
    for w in times.windows(2) {
        assert!(w[1] - w[0] >= 3145, "{times:?}");
    }
}

#[test]
fn ds2_frequent() {
    let mock = Mock::new();
    let mut c = core(&mock, DS2);
    let resp = xor(&[0x12, 0x05, 0xA0, 0x88]);
    c.frequent_mode(&[0x12, 0x04, 0x00]).unwrap();
    assert_eq!(c.thread_command, ThreadCommand::IdleTransmit);
    mock.expect(&xor(&[0x12, 0x04, 0x00]), &[(40, &resp)]);
    c.idle_transmit();
    assert_eq!(c.single_transmit(&[]).unwrap(), resp);
    // nothing new: next read transmits again
    mock.expect(&xor(&[0x12, 0x04, 0x00]), &[(40, &resp)]);
    assert_eq!(c.single_transmit(&[]).unwrap(), resp);
    c.frequent_mode(&[]).unwrap();
    assert_eq!(c.thread_command, ThreadCommand::Idle);
}

// ---------------------------------------------------------------------------
// KWP2000*
// ---------------------------------------------------------------------------

#[test]
fn kwp2000s_request_response() {
    let mock = Mock::new();
    let mut c = core(&mock, KWP2000S);
    assert_eq!(mock.st().events[0], Ev::Config(9600, 8, Parity::Even));
    let resp = xor(&[0xB8, 0xF1, 0x12, 0x03, 0x5A, 0x80, 0x01]);
    mock.expect(&xor(&[0xB8, 0x12, 0xF1, 0x02, 0x1A, 0x80]), &[(30, &resp)]);
    assert_eq!(
        c.obd_trans(&[0xB8, 0x12, 0xF1, 0x02, 0x1A, 0x80]).unwrap(),
        resp
    );
}

#[test]
fn kwp2000s_nr78_and_checksum() {
    let mock = Mock::new();
    let mut c = core(&mock, KWP2000S);
    let pending = xor(&[0xB8, 0xF1, 0x12, 0x03, 0x7F, 0x31, 0x78]);
    let resp = xor(&[0xB8, 0xF1, 0x12, 0x02, 0x71, 0x01]);
    mock.expect(
        &xor(&[0xB8, 0x12, 0xF1, 0x02, 0x31, 0x01]),
        &[(30, &pending), (2500, &resp)],
    );
    assert_eq!(
        c.obd_trans(&[0xB8, 0x12, 0xF1, 0x02, 0x31, 0x01]).unwrap(),
        resp
    );

    let mut bad = resp.clone();
    *bad.last_mut().unwrap() ^= 0x55;
    mock.expect(&xor(&[0xB8, 0x12, 0xF1, 0x02, 0x31, 0x01]), &[(30, &bad)]);
    assert_eq!(
        c.obd_trans(&[0xB8, 0x12, 0xF1, 0x02, 0x31, 0x01]),
        Err(ErrorCode::EDIABAS_IFH_0009)
    );
}

// ---------------------------------------------------------------------------
// KWP2000 BMW
// ---------------------------------------------------------------------------

#[test]
fn kwp2000_bmw_fast_init_start_comm_and_tester_present() {
    let mock = Mock::new();
    let mut c = core(&mock, &kwp2000_bmw_pars());
    assert_eq!(c.transmit, Some(Transmit::Kwp2000Bmw));
    assert!(c.ecu_connected);
    // the setter marks the ECU connected: first request without init fails
    let req = sum(&[0x82, 0x12, 0xF1, 0x1A, 0x80]);
    mock.expect(&req, &[]);
    assert_eq!(
        c.trans_kwp2000_bmw(&[0x82, 0x12, 0xF1, 0x1A, 0x80]),
        Err(ErrorCode::EDIABAS_IFH_0009)
    );
    assert!(!c.ecu_connected);

    // retry: fast init + start communication
    let t_req = mock
        .st()
        .events
        .iter()
        .find_map(|e| {
            if let Ev::Write(_, t) = e {
                Some(*t)
            } else {
                None
            }
        })
        .unwrap();
    mock.st().events.clear();
    let sc_resp = sum(&[0x83, 0xF1, 0x12, 0xC1, 0xEF, 0x8F]);
    let resp = sum(&[0x83, 0xF1, 0x12, 0x5A, 0x80, 0x00]);
    mock.expect(&sum(&[0x81, 0x12, 0xF1, 0x81]), &[(30, &sc_resp)]);
    mock.expect(&req, &[(30, &resp)]);
    assert_eq!(c.obd_trans(&[0x82, 0x12, 0xF1, 0x1A, 0x80]).unwrap(), resp);
    assert!(c.ecu_connected);

    let ev = mock.st().events.clone();
    // waits timeout std since the last communication
    let Ev::Break(true, t0) = ev[0] else {
        panic!("{ev:?}")
    };
    assert!(t0 - t_req >= 1_000_000, "{t0} {t_req}");
    assert_eq!(ev[1], Ev::Break(false, t0 + 25_000));
    assert_eq!(ev[2], Ev::Purge(t0 + 50_000));
    assert_eq!(
        ev[3],
        Ev::Write(sum(&[0x81, 0x12, 0xF1, 0x81]), t0 + 50_000)
    );

    // tester present after 2000 ms idle
    c.thread_command = ThreadCommand::IdleTransmit;
    c.idle_transmit();
    assert_eq!(mock.writes().len(), 2);
    mock.st().now += 2_001_000;
    mock.expect(
        &sum(&[0x81, 0x12, 0xF1, 0x3E]),
        &[(20, &sum(&[0x81, 0xF1, 0x12, 0x7E]))],
    );
    c.idle_transmit();
    assert_eq!(mock.writes().len(), 3);
    assert_eq!(c.thread_command, ThreadCommand::IdleTransmit);
    assert!(mock.st().unexpected.is_empty());

    // tester present without answer: ECU disconnected, idle stops
    mock.st().now += 2_001_000;
    mock.expect(&sum(&[0x81, 0x12, 0xF1, 0x3E]), &[]);
    c.idle_transmit();
    assert!(!c.ecu_connected);
    assert_eq!(c.thread_command, ThreadCommand::Idle);
}

#[test]
fn kwp2000_bmw_start_comm_failure() {
    let mock = Mock::new();
    let mut c = core(&mock, &kwp2000_bmw_pars());
    c.ecu_connected = false;
    mock.expect(&sum(&[0x81, 0x12, 0xF1, 0x81]), &[]);
    assert_eq!(
        c.trans_kwp2000_bmw(&[0x82, 0x12, 0xF1, 0x1A, 0x80]),
        Err(ErrorCode::EDIABAS_IFH_0009)
    );
    assert!(!c.ecu_connected);
    assert_eq!(mock.writes().len(), 1);
}

// ---------------------------------------------------------------------------
// ISO KWP2000 with 5 baud init
// ---------------------------------------------------------------------------

fn iso_core(mock: &Mock) -> ObdCore {
    let mut c = ObdCore::new();
    c.link = Some(Box::new(mock.clone()));
    c.has_adapter_echo = false;
    c.has_auto_baud_rate = false;
    c.transmit = Some(Transmit::Kwp2000);
    c.comm_par = Some(vec![0]);
    c.par_edic_wake_address = 0x33;
    c.par_edic_ecu_address = 0x33;
    c.par_edic_tester_address = 0xF1;
    c.par_edic_w1 = 300;
    c.par_edic_w2 = 20;
    c.par_edic_w3 = 20;
    c.par_edic_w4a = 25;
    c.par_edic_w4 = 50;
    c.par_timeout_std = 100;
    c.par_timeout_tel_end = 20;
    c
}

#[test]
fn kwp2000_iso_5baud_init_and_request() {
    let mock = Mock::new();
    mock.st().echo = false;
    let mut c = iso_core(&mock);
    let t0 = mock.now();
    // 5 baud address: 10 bits * 200 ms, then the ECU answers
    mock.rx_at(t0 + 2_050_000, &[0x55]);
    mock.rx_at(t0 + 2_060_000, &[0x08, 0x8F]);
    mock.expect(&[0x70], &[(30, &[0xCC])]);
    let resp = sum(&[0x83, 0xF1, 0x33, 0x5A, 0x80, 0x00]);
    mock.expect(&sum(&[0x82, 0x33, 0xF1, 0x1A, 0x80]), &[(30, &resp)]);
    assert_eq!(c.obd_trans(&[0x1A, 0x80]).unwrap(), resp);
    assert!(c.ecu_connected);
    assert_eq!(c.kwp_mode, KwpMode::Kwp2000);
    assert_eq!(c.key_bytes, vec![0x08, 0x8F, 0xCC, 0xA0, 0x28]);
    assert!(mock.st().unexpected.is_empty());

    // address 0x33 LSB first: start bit, 1 1 0 0 1 1 0 0, stop bit
    let breaks: Vec<(bool, u64)> = mock
        .st()
        .events
        .iter()
        .filter_map(|e| {
            if let Ev::Break(on, t) = e {
                Some((*on, *t))
            } else {
                None
            }
        })
        .collect();
    let bits: Vec<bool> = breaks.iter().map(|b| b.0).collect();
    assert_eq!(
        bits,
        vec![
            true, false, false, true, true, false, false, true, true, false
        ]
    );
    for (i, (_, t)) in breaks.iter().enumerate() {
        assert_eq!(*t - breaks[0].1, i as u64 * 200_000);
    }
}

#[test]
fn kwp2000_iso_wrong_wake_address_and_finish() {
    let mock = Mock::new();
    mock.st().echo = false;
    let mut c = iso_core(&mock);
    let t0 = mock.now();
    mock.rx_at(t0 + 2_050_000, &[0x55]);
    mock.rx_at(t0 + 2_060_000, &[0x08, 0x8F]);
    mock.expect(&[0x70], &[(30, &[0xCD])]);
    assert_eq!(
        c.trans_kwp2000_iso(&[0x1A, 0x80]),
        Err(ErrorCode::EDIABAS_IFH_0009)
    );
    assert!(!c.ecu_connected);

    // connected ECU: finish sends stop communication
    c.ecu_connected = true;
    c.kwp_mode = KwpMode::Kwp2000;
    mock.expect(
        &sum(&[0x81, 0x33, 0xF1, 0x82]),
        &[(20, &sum(&[0x81, 0xF1, 0x33, 0xC2]))],
    );
    assert_eq!(c.obd_finish_trans(), Ok(()));
    assert!(!c.ecu_connected);
}

#[test]
fn kwp2000_iso_no_wake_response() {
    let mock = Mock::new();
    let mut c = iso_core(&mock);
    assert_eq!(
        c.trans_kwp2000_iso(&[0x1A, 0x80]),
        Err(ErrorCode::EDIABAS_IFH_0009)
    );
}

// ---------------------------------------------------------------------------
// ObdInterface
// ---------------------------------------------------------------------------

fn iface(mock: &Mock) -> ObdInterface {
    let m = mock.clone();
    let mut i = ObdInterface::with_opener(
        "mock",
        Box::new(move |_| Ok(Box::new(m.clone()) as Box<dyn SerialLink>)),
    );
    i.set_use_comm_thread(false);
    i
}

#[test]
fn interface_connect_transmit_disconnect() {
    let mock = Mock::new();
    let mut i = iface(&mock);
    assert!(!i.connected());
    assert_eq!(i.transmit_data(&[1]), Err(ErrorCode::EDIABAS_IFH_0006));
    i.interface_connect().unwrap();
    assert!(i.connected());
    assert_eq!(i.transmit_data(&[1]), Err(ErrorCode::EDIABAS_IFH_0006));
    i.set_comm_parameter(BMW_FAST.to_vec()).unwrap();
    assert_eq!(i.comm_parameter(), BMW_FAST);
    let resp = sum(&[0x83, 0xF1, 0x12, 0x5A, 0x80, 0x00]);
    mock.expect(&sum(&[0x82, 0x12, 0xF1, 0x1A, 0x80]), &[(30, &resp)]);
    assert_eq!(
        i.transmit_data(&[0x82, 0x12, 0xF1, 0x1A, 0x80]).unwrap(),
        resp
    );
    assert_eq!(
        i.transmit_data(&vec![0; 0x801]),
        Err(ErrorCode::EDIABAS_IFH_0031)
    );
    assert_eq!(i.state().unwrap(), vec![0x00, 0x00]);
    mock.st().dsr = false;
    assert_eq!(i.ignition_voltage().unwrap(), 0);
    assert_eq!(i.raw_data(&[0xF1, 0x07]).unwrap(), vec![0x05, 0x03, 0x00]);
    let log = i.log_handle().lock().unwrap().join("\n");
    assert!(log.contains("Resp: 83 F1 12 5A 80 00"), "{log}");
    i.interface_disconnect().unwrap();
    assert!(!i.connected());
    assert_eq!(i.state(), Err(ErrorCode::EDIABAS_IFH_0056));
}

#[test]
fn interface_connect_fails() {
    let mut i = ObdInterface::with_opener("x", Box::new(|_| Err(std::io::Error::other("no port"))));
    assert_eq!(i.interface_connect(), Err(ErrorCode::EDIABAS_IFH_0018));
    let mut i = ObdInterface::new("");
    assert_eq!(i.interface_connect(), Err(ErrorCode::EDIABAS_IFH_0018));
}

#[test]
fn interface_ds2_answer_len_and_frequent() {
    let mock = Mock::new();
    let mut i = iface(&mock);
    i.interface_connect().unwrap();
    i.set_comm_parameter(DS2.to_vec()).unwrap();
    assert_eq!(i.comm_answer_len(), &[-1, 0]);
    i.set_comm_answer_len(vec![3, 0]).unwrap();
    let resp = xor(&[0xA0, 0x01]);
    mock.expect(&xor(&[0x12, 0x04, 0x00]), &[(40, &resp)]);
    i.transmit_frequent(&[0x12, 0x04, 0x00]).unwrap();
    assert_eq!(i.receive_frequent().unwrap(), resp);
    i.stop_frequent().unwrap();
    assert!(i.core().send_buffer_frequent.is_empty());
}

#[test]
fn interface_with_comm_thread_sends_tester_present() {
    let mock = Mock::new();
    let m = mock.clone();
    let mut i = ObdInterface::with_opener(
        "mock",
        Box::new(move |_| Ok(Box::new(m.clone()) as Box<dyn SerialLink>)),
    );
    i.interface_connect().unwrap();
    i.set_comm_parameter(kwp2000_bmw_pars()).unwrap();
    let req = sum(&[0x82, 0x12, 0xF1, 0x1A, 0x80]);
    let resp = sum(&[0x83, 0xF1, 0x12, 0x5A, 0x80, 0x00]);
    mock.expect(&req, &[(30, &resp)]);
    assert_eq!(
        i.transmit_data(&[0x82, 0x12, 0xF1, 0x1A, 0x80]).unwrap(),
        resp
    );
    // simulated time jumps past the tester present time; the thread sends it
    mock.expect(
        &sum(&[0x81, 0x12, 0xF1, 0x3E]),
        &[(20, &sum(&[0x81, 0xF1, 0x12, 0x7E]))],
    );
    mock.st().now += 2_001_000;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while mock.writes().len() < 2 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(mock.writes().len(), 2);
    i.interface_disconnect().unwrap();
}

//! Property tests: decode(encode(x)) == x for every valid header / question.

use std::net::Ipv4Addr;

use nebula_wire::{
    Flags, Header, Message, Name, OpCode, QClass, QType, Question, RCode, RData, ResourceRecord,
    HEADER_LEN,
};
use proptest::prelude::*;

fn opcode_strategy() -> impl Strategy<Value = OpCode> {
    prop_oneof![
        Just(OpCode::Query),
        Just(OpCode::IQuery),
        Just(OpCode::Status),
        Just(OpCode::Notify),
        Just(OpCode::Update),
    ]
}

fn rcode_strategy() -> impl Strategy<Value = RCode> {
    (0u8..=10u8).prop_map(RCode::from_u8)
}

fn flags_strategy() -> impl Strategy<Value = Flags> {
    (
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(qr, aa, tc, rd, ra, ad, cd)| Flags {
            qr,
            aa,
            tc,
            rd,
            ra,
            ad,
            cd,
        })
}

fn header_strategy() -> impl Strategy<Value = Header> {
    (
        any::<u16>(),
        flags_strategy(),
        opcode_strategy(),
        rcode_strategy(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
    )
        .prop_map(|(id, flags, opcode, rcode, qd, an, ns, ar)| Header {
            id,
            flags,
            opcode,
            rcode,
            qdcount: qd,
            ancount: an,
            nscount: ns,
            arcount: ar,
        })
}

fn label_strategy() -> impl Strategy<Value = String> {
    // ASCII alphanumeric labels of length 1..=10, to stay well under limits.
    "[a-zA-Z0-9]{1,10}"
}

fn name_strategy() -> impl Strategy<Value = Name> {
    prop::collection::vec(label_strategy(), 0..8).prop_map(|labels| {
        if labels.is_empty() {
            Name::root()
        } else {
            Name::from_ascii(&labels.join(".")).unwrap()
        }
    })
}

fn qtype_strategy() -> impl Strategy<Value = QType> {
    any::<u16>().prop_map(QType)
}

fn qclass_strategy() -> impl Strategy<Value = QClass> {
    any::<u16>().prop_map(QClass)
}

fn question_strategy() -> impl Strategy<Value = Question> {
    (name_strategy(), qtype_strategy(), qclass_strategy()).prop_map(|(qname, qtype, qclass)| {
        Question {
            qname,
            qtype,
            qclass,
        }
    })
}

fn a_record_strategy() -> impl Strategy<Value = ResourceRecord> {
    (name_strategy(), any::<u32>(), any::<[u8; 4]>()).prop_map(|(name, ttl, octets)| {
        ResourceRecord {
            name,
            class: QClass::IN,
            ttl,
            data: RData::A(Ipv4Addr::from(octets)),
        }
    })
}

fn message_strategy() -> impl Strategy<Value = Message> {
    (
        header_strategy(),
        prop::collection::vec(question_strategy(), 0..4),
        prop::collection::vec(a_record_strategy(), 0..4),
    )
        .prop_map(|(mut header, questions, answers)| {
            // The encoder rewrites the counts; initialize them anyway for clarity.
            header.qdcount = u16::try_from(questions.len()).unwrap_or(u16::MAX);
            header.ancount = u16::try_from(answers.len()).unwrap_or(u16::MAX);
            header.nscount = 0;
            header.arcount = 0;
            Message {
                header,
                questions,
                answers,
                authority: Vec::new(),
                additional: Vec::new(),
                edns: None,
            }
        })
}

proptest! {
    #[test]
    fn header_roundtrip(h in header_strategy()) {
        let mut buf = [0u8; HEADER_LEN];
        h.encode(&mut buf).unwrap();
        let decoded = Header::decode(&buf).unwrap();
        prop_assert_eq!(h, decoded);
    }

    #[test]
    fn question_roundtrip(q in question_strategy()) {
        let mut buf = vec![0u8; q.wire_len()];
        q.encode(&mut buf).unwrap();
        let (decoded, end) = Question::decode(&buf, 0).unwrap();
        prop_assert_eq!(decoded, q);
        prop_assert_eq!(end, buf.len());
    }

    #[test]
    fn decode_never_panics_on_arbitrary_bytes(bytes in prop::collection::vec(any::<u8>(), 0..1024)) {
        // Decoder must never panic regardless of input.
        let _ = Header::decode(&bytes);
        let _ = Question::decode(&bytes, 0);
        let _ = Message::decode(&bytes);
    }

    #[test]
    fn message_roundtrip(msg in message_strategy()) {
        let mut buf = vec![0u8; 4096];
        let n = msg.encode(&mut buf).unwrap();
        let back = Message::decode(&buf[..n]).unwrap();
        prop_assert_eq!(back.questions, msg.questions);
        prop_assert_eq!(back.answers, msg.answers);
    }
}

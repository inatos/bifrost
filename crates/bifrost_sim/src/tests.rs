use crate::input::FrameInput;
use crate::{checksum, new_match, simulate_frames, step};

#[test]
fn identical_inputs_produce_identical_checksums() {
    let seed = 42u64;
    let inputs: Vec<FrameInput> = (0..240)
        .map(|i| FrameInput {
            p0: if i % 3 == 0 { 1 } else { 0 },
            p1: if i % 5 == 0 { 2 } else { 0 },
        })
        .collect();
    let a = checksum(&simulate_frames(seed, &inputs));
    let b = checksum(&simulate_frames(seed, &inputs));
    assert_eq!(a, b);
}

#[test]
fn replay_roundtrip() {
    let replay = crate::Replay {
        seed: 7,
        inputs: vec![FrameInput::default(); 10],
    };
    let code = crate::encode_replay(&replay);
    let decoded = crate::decode_replay(&code).expect("decode");
    assert_eq!(replay, decoded);
}

#[test]
fn checkpoint_resume_matches_continuous_run() {
    let seed = 99;
    let inputs: Vec<FrameInput> = (0..120)
        .map(|i| FrameInput {
            p0: if i % 4 == 0 { 1 } else { 0 },
            p1: if i % 6 == 0 { 2 } else { 0 },
        })
        .collect();

    let full = simulate_frames(seed, &inputs);

    let mut partial = new_match(seed);
    for inp in inputs.iter().take(60) {
        step(&mut partial, *inp);
    }
    let checkpoint = partial.clone();
    for inp in inputs.iter().skip(60) {
        step(&mut partial, *inp);
    }

    let mut resumed = checkpoint;
    for inp in inputs.iter().skip(60) {
        step(&mut resumed, *inp);
    }

    assert_eq!(checksum(&full), checksum(&partial));
    assert_eq!(checksum(&partial), checksum(&resumed));
}

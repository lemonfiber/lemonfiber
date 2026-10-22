use super::sha256;

#[test]
fn a_digest_is_sha_256_in_lower_case_hexadecimal() {
    assert_eq!(
        sha256(""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

use argon2::{Algorithm, Argon2, Params, Version};

fn main() {
    let password = "test_password_123";
    let salt = [0u8; 32];
    let params = Params::new(65536, 3, 4, Some(32)).unwrap();
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; 32];

    let start = std::time::Instant::now();
    argon2
        .hash_password_into(password.as_bytes(), &salt, &mut key)
        .unwrap();
    let t1 = start.elapsed();
    println!("Argon2id call #1: {:?}", t1);

    let start = std::time::Instant::now();
    argon2
        .hash_password_into(password.as_bytes(), &salt, &mut key)
        .unwrap();
    let t2 = start.elapsed();
    println!("Argon2id call #2: {:?}", t2);

    let start = std::time::Instant::now();
    argon2
        .hash_password_into(password.as_bytes(), &salt, &mut key)
        .unwrap();
    let t3 = start.elapsed();
    println!("Argon2id call #3: {:?}", t3);

    println!("Total 3 calls: {:?}", t1 + t2 + t3);
}

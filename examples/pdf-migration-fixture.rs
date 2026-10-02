#[path = "../tests/support/pdf_fixture.rs"]
mod fixture;
fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("provide an explicit fixture output path");
    std::fs::write(path, fixture::document_bytes()).expect("write synthetic fixture");
}

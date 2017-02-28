error_chain! {
    foreign_links {
        Io(::std::io::Error);
    }

    errors {
        InvalidProgram(m: String) {
            description("invalid program")
            display("invalid program '{}'", m)
        }
    }
}

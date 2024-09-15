#[macro_export]
macro_rules! log_format {
    ($fmt_str:literal) => {
        {
            let _ = ::std::io::Write::write_all(
                &mut ::std::io::stderr(),
                format!(concat!($fmt_str, "\n")).as_bytes()
            );
        }
    };
    ($fmt_str:literal, $($args:expr),*) => {
        {
            let _ = ::std::io::Write::write_all(
                &mut ::std::io::stderr(),
                format!(concat!($fmt_str, "\n"), $($args),*).as_bytes()
            );
        }
    };
}

#[macro_export]
macro_rules! format_error {
    ($($args:expr),*) => {
        ::std::io::Error::new(::std::io::ErrorKind::Other, format!($($args),*))
    };
}

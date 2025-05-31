#[macro_export]
macro_rules! select_continue {
    ($($fut:expr),+ $(,)?) => {{
        use tokio::signal::ctrl_c;

        tokio::select! {
            $(
                res = $fut => {
                    match res {
                        Ok(_) => continue,
                        Err(_) => break,
                    }
                },
            )+
            _ = ctrl_c() => {
                break;
            }
        };
    }};
}

pub fn get_thread_number(num_config: Option<i64>) -> usize {
    if num_config.is_none() {
        return num_cpus::get_physical() - 1;
    }
    let num = num_config.unwrap();

    usize::try_from(num).unwrap_or(1)
}

use crate::CommunityError;

pub(crate) fn parallel_collect<T, F>(
    count: usize,
    requested_workers: usize,
    operation: F,
) -> Result<Vec<T>, CommunityError>
where
    T: Send,
    F: Fn(usize) -> Result<T, CommunityError> + Sync,
{
    if count == 0 {
        return Ok(Vec::new());
    }
    let workers = requested_workers.max(1).min(count);
    if workers == 1 {
        return (0..count).map(operation).collect();
    }

    let chunk = count.div_ceil(workers);
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        let operation = &operation;
        for start in (0..count).step_by(chunk) {
            let end = (start + chunk).min(count);
            handles.push(scope.spawn(move || {
                (start..end)
                    .map(|index| operation(index))
                    .collect::<Result<Vec<_>, _>>()
            }));
        }
        let mut output = Vec::with_capacity(count);
        for handle in handles {
            let mut values = handle.join().map_err(|_| {
                CommunityError::Leiden("community scan worker panicked".to_owned())
            })??;
            output.append(&mut values);
        }
        Ok(output)
    })
}

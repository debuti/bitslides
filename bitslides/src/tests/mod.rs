use tempfile::tempdir;

use crate::main_w_args;

#[tokio::test]
async fn test_main_dummy_environment() {
    let temp_dir = tempdir().unwrap();
    // Use forward slashes in Windows
    let temp_dir_str = temp_dir.path().to_str().unwrap().replace("\\", "/");
    let config_file = temp_dir.path().join("config.yml");
    let config_content = format!(
        r#"
# Example configuration file
keyword: "slides"
roots:
- "root0"
- "root1"
trace: "{}/bitslides.%Y%M%d%H%M%S.log"
"#,
        temp_dir_str
    );
    std::fs::write(&config_file, config_content).unwrap();

    let args = vec!["bitslides", "-c", config_file.to_str().unwrap()];

    // Create a channel and spawn a task to send shutdown signal after a delay
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let _ = shutdown_tx.send(());
    });

    assert!(main_w_args(
        args.into_iter()
            .map(|x| x.to_owned())
            .collect::<Vec<String>>()
            .as_slice(),
        shutdown_rx,
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_main_corrupted_config() {
    let temp_dir = tempdir().unwrap();
    let config_file = temp_dir.path().join("config.yml");
    let config_content = r#"Memento mori"#;
    std::fs::write(&config_file, config_content).unwrap();

    let args = vec!["bitslides", "-c", config_file.to_str().unwrap()];

    // Create a channel and spawn a task to send shutdown signal after a delay
    // (though the test will error out before reaching the signal wait)
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let _ = shutdown_tx.send(());
    });

    assert!(main_w_args(
        args.into_iter()
            .map(|x| x.to_owned())
            .collect::<Vec<String>>()
            .as_slice(),
        shutdown_rx,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn test_main_missing_config() {
    let args = vec!["bitslides", "-c", "not-to-be-found"];

    // Create a channel and spawn a task to send shutdown signal after a delay
    // (though the test will error out before reaching the signal wait)
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let _ = shutdown_tx.send(());
    });

    assert!(main_w_args(
        args.into_iter()
            .map(|x| x.to_owned())
            .collect::<Vec<String>>()
            .as_slice(),
        shutdown_rx,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn test_main_wrong_trace_config() {
    let temp_dir = tempdir().unwrap();
    let temp_dir_str = temp_dir.path().to_str().unwrap().replace("\\", "/");
    let config_file = temp_dir.path().join("config.yml");
    let config_content = format!(
        r#"
# Example configuration file
keyword: "slides"
roots:
- "root0"
- "root1"
trace: "{}/non-existing-folder/bitslides.log"
"#,
        temp_dir_str
    );
    std::fs::write(&config_file, config_content).unwrap();

    let args = vec!["bitslides", "-c", config_file.to_str().unwrap()];

    // Create a channel and spawn a task to send shutdown signal after a delay
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let _ = shutdown_tx.send(());
    });

    let result = main_w_args(
        args.into_iter()
            .map(|x| x.to_owned())
            .collect::<Vec<String>>()
            .as_slice(),
        shutdown_rx,
    )
    .await;

    assert!(result.is_ok(), "Failed with: {}", result.unwrap_err());
}

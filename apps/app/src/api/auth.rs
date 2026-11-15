use crate::api::Result;
use std::time::Duration;
use tauri::plugin::TauriPlugin;
use tauri::{Runtime, Url};
use tauri_plugin_opener::OpenerExt;
use theseus::prelude::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("auth")
        .invoke_handler(tauri::generate_handler![
            check_reachable,
            login,
            remove_user,
            get_default_user,
            set_default_user,
            get_users,
        ])
        .build()
}

/// Checks if the authentication servers are reachable.
#[tauri::command]
pub async fn check_reachable() -> Result<()> {
    minecraft_auth::check_reachable().await?;
    Ok(())
}

/// Öffnet den Microsoft-Login im Standardbrowser und wartet auf den Rücksprung
/// an einen lokalen Port (OAuth Authorization Code + PKCE, Loopback-Redirect).
#[tauri::command]
pub async fn login<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<Credentials>> {
    let other = |msg: String| theseus::ErrorKind::OtherError(msg).as_error();

    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| {
        other(format!("Could not start local login listener: {e}"))
    })?;
    let port = listener
        .local_addr()
        .map_err(|e| other(e.to_string()))?
        .port();
    let redirect_uri = format!("http://localhost:{port}");

    let flow = minecraft_auth::begin_login(&redirect_uri).await?;

    app.opener()
        .open_url(&flow.auth_request_uri, None::<&str>)
        .map_err(|e| other(format!("Could not open browser: {e}")))?;

    let code = match tokio::time::timeout(
        Duration::from_secs(600),
        wait_for_code(&listener, &flow.session_id),
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => return Ok(None), // Timeout nach 10 Minuten
    };

    let val = minecraft_auth::finish_login(&code, flow).await?;
    Ok(Some(val))
}

async fn wait_for_code(
    listener: &TcpListener,
    expected_state: &str,
) -> theseus::Result<String> {
    let other = |msg: String| theseus::ErrorKind::OtherError(msg).as_error();

    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| other(e.to_string()))?;

        let mut buf = vec![0u8; 8192];
        let n = match tokio::time::timeout(
            Duration::from_secs(5),
            stream.read(&mut buf),
        )
        .await
        {
            Ok(Ok(n)) if n > 0 => n,
            _ => continue,
        };

        let request = String::from_utf8_lossy(&buf[..n]).to_string();
        let target = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");

        let Ok(url) = Url::parse(&format!("http://localhost{target}")) else {
            respond(&mut stream, "400 Bad Request", "Ungültige Anfrage.")
                .await;
            continue;
        };

        if url.path() != "/" {
            respond(&mut stream, "404 Not Found", "").await;
            continue;
        }

        let param = |name: &str| {
            url.query_pairs()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.to_string())
        };

        if let Some(error) = param("error") {
            let description = param("error_description").unwrap_or_default();
            respond(
                &mut stream,
                "200 OK",
                "<h2>Anmeldung fehlgeschlagen</h2><p>Du kannst dieses Fenster schließen und es in Aperion erneut versuchen.</p>",
            )
            .await;
            return Err(other(format!(
                "Microsoft login error: {error} {description}"
            )));
        }

        match (param("code"), param("state")) {
            (Some(code), Some(state)) if state == expected_state => {
                respond(
                    &mut stream,
                    "200 OK",
                    "<h2>Anmeldung erfolgreich</h2><p>Du kannst dieses Fenster schließen und zu Aperion zurückkehren.</p>",
                )
                .await;
                return Ok(code);
            }
            _ => {
                respond(&mut stream, "400 Bad Request", "Ungültige Anfrage.")
                    .await;
            }
        }
    }
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Aperion</title></head><body style=\"font-family:sans-serif;text-align:center;margin-top:20vh\">{body}</body></html>"
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

#[tauri::command]
pub async fn remove_user(user: uuid::Uuid) -> Result<()> {
    Ok(minecraft_auth::remove_user(user).await?)
}

#[tauri::command]
pub async fn get_default_user() -> Result<Option<uuid::Uuid>> {
    Ok(minecraft_auth::get_default_user().await?)
}

#[tauri::command]
pub async fn set_default_user(user: uuid::Uuid) -> Result<()> {
    Ok(minecraft_auth::set_default_user(user).await?)
}

/// Get a copy of the list of all user credentials
#[tauri::command]
pub async fn get_users() -> Result<Vec<Credentials>> {
    Ok(minecraft_auth::users().await?)
}

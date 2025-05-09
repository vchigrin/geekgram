use super::storage::Storage;
use color_eyre::Result;
use grammers_client::{session::Session, Client, Config, SignInError};
use std::io::{BufRead, Write};

// TODO: At present most of the code here copied from grammers-client examples.
// We should made sign-in process more user-friendly. Going to re-write UI
// here with some Ratatui controls.
pub struct TgClientBuilder {}

const API_ID: i32 = match i32::from_str_radix(env!("TG_ID"), 10) {
    Ok(v) => v,
    Err(_) => {
        panic!("Invalid TG_ID environment variable")
    }
};
const API_HASH: &str = env!("TG_HASH");

impl TgClientBuilder {
    fn prompt(message: &str) -> Result<String> {
        let stdout = std::io::stdout();
        let mut stdout = stdout.lock();
        stdout.write_all(message.as_bytes())?;
        stdout.flush()?;

        let stdin = std::io::stdin();
        let mut stdin = stdin.lock();

        let mut line = String::new();
        stdin.read_line(&mut line)?;
        Ok(line)
    }

    pub async fn make_signed_in_client(storage: &Storage) -> Result<Client> {
        let session;
        if let Ok(sess) = storage.load_session() {
            session = sess;
        } else {
            session = Session::new();
        }
        let client = Client::connect(Config {
            session,
            api_id: API_ID,
            api_hash: API_HASH.to_string(),
            params: Default::default(),
        })
        .await?;

        if !client.is_authorized().await? {
            let phone = Self::prompt("Enter your phone number (international format): ")?;
            let token = client.request_login_code(&phone).await?;
            let code = Self::prompt("Enter the code you received: ")?;
            let signed_in = client.sign_in(&token, &code).await;
            match signed_in {
                Err(SignInError::PasswordRequired(password_token)) => {
                    // Note: this `prompt` method will echo the password in the console.
                    //       Real code might want to use a better way to handle this.
                    let hint = password_token.hint().unwrap_or("None");
                    let prompt_message = format!("Enter the password (hint {}): ", &hint);
                    let password = Self::prompt(prompt_message.as_str())?;

                    client
                        .check_password(password_token, password.trim())
                        .await?;
                }
                Ok(_) => (),
                Err(e) => panic!("{}", e),
            };
            log::info!("Signed in!");
        }
        storage.save_session(client.session())?;
        Ok(client)
    }
}

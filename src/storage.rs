use color_eyre::Result;
use eyre::eyre;
use grammers_client::session::Session;
use grammers_client::types::{Channel, Chat, Dialog, Group, User};
use grammers_tl_types as tl_types;
use grammers_tl_types::Cursor;
use grammers_tl_types::Deserializable;
use grammers_tl_types::Serializable;

pub struct Storage {
    connection: rusqlite::Connection,
}

impl Storage {
    pub fn new(db_file_path: &std::path::Path) -> Result<Self> {
        let connection = rusqlite::Connection::open(db_file_path)?;
        Self::ensure_blob_table(&connection, "users")?;
        Self::ensure_blob_table(&connection, "groups")?;
        Self::ensure_blob_table(&connection, "channels")?;
        Self::ensure_blob_table(&connection, "dialogs")?;
        Self::ensure_blob_table(&connection, "session")?;
        let result = Self { connection };
        Ok(result)
    }

    fn ensure_blob_table(connection: &rusqlite::Connection, table_name: &str) -> Result<()> {
        let statement = format!(
            "CREATE TABLE IF NOT EXISTS {}(id INTEGER PRIMARY KEY, data BLOB);",
            table_name
        );
        connection.execute(&statement, ())?;
        Ok(())
    }

    fn to_bot_id(chat: &Chat) -> i64 {
        match chat {
            Chat::User(user) => {
                assert!(user.id() >= 0);
                assert!(user.id() <= 0xffffffffff);
                user.id()
            }
            Chat::Group(group) => {
                assert!(group.id() >= 1);
                assert!(group.id() <= 999999999999);
                -group.id()
            }
            Chat::Channel(channel) => {
                assert!(channel.id() >= 1);
                assert!(channel.id() <= 997852516352);
                -(1000000000000 + channel.id())
            }
        }
    }

    pub fn save_dialog(&self, dialog: &Dialog) -> Result<()> {
        self.save_chat(&dialog.chat)?;
        self.save_generic("dialogs", Self::to_bot_id(&dialog.chat), &dialog.raw)
    }

    pub fn select_all_dialogs(&self) -> Result<Vec<Dialog>> {
        let mut select_all_stmt = self
            .connection
            .prepare_cached("SELECT id, data FROM dialogs;")?;
        let mut rows = select_all_stmt.query([])?;
        let mut result = Vec::new();
        while let Some(row) = rows.next()? {
            let id = row.get::<usize, i64>(0)?;
            let data = row.get::<usize, Vec<u8>>(1)?;
            let raw_dlg =
                grammers_tl_types::enums::Dialog::deserialize(&mut Cursor::from_slice(&data))?;
            let chat = self.load_chat(raw_dlg.peer())?;
            if Self::to_bot_id(&chat) != id {
                return Err(eyre!("DB damaged, ID mismatch"));
            }
            let dlg = Dialog {
                raw: raw_dlg,
                chat,
                last_message: None,
            };
            result.push(dlg);
        }

        Ok(result)
    }

    fn save_chat(&self, chat: &Chat) -> Result<()> {
        match chat {
            Chat::User(usr) => self.save_user(usr),
            Chat::Group(grp) => self.save_group(grp),
            Chat::Channel(chn) => self.save_channel(chn),
        }
    }

    fn save_generic<T>(&self, table_name: &str, id: i64, obj: &T) -> Result<()>
    where
        T: Serializable,
    {
        let statement = format!(
            "INSERT OR REPLACE INTO {}(id, data) VALUES (?, ?);",
            table_name
        );
        let mut cached_statement = self.connection.prepare_cached(&statement)?;
        let serialized = obj.to_bytes();
        cached_statement.execute((id, serialized))?;
        Ok(())
    }

    pub fn save_user(&self, user: &User) -> Result<()> {
        self.save_generic("users", user.id(), &user.raw)
    }

    pub fn save_group(&self, group: &Group) -> Result<()> {
        self.save_generic("groups", group.id(), &group.raw)
    }

    pub fn save_channel(&self, channel: &Channel) -> Result<()> {
        self.save_generic("channels", channel.id(), &channel.raw)
    }

    fn load_chat(&self, peer: tl_types::enums::Peer) -> Result<Chat> {
        // NOTE, that ID sequences for users, chats and channels, overlap
        // (that stated by Telegram API documentation),
        // so we must use three separate tables.
        match peer {
            tl_types::enums::Peer::User(peer_user) => {
                let result = self.load_user(peer_user)?;
                Ok(Chat::User(result))
            }
            tl_types::enums::Peer::Chat(peer_group) => {
                let result = self.load_group(peer_group)?;
                Ok(Chat::Group(result))
            }
            tl_types::enums::Peer::Channel(peer_channel) => {
                let result = self.load_channel(peer_channel)?;
                Ok(Chat::Channel(result))
            }
        }
    }

    fn load_user(&self, user: tl_types::types::PeerUser) -> Result<User> {
        let mut select_stmt = self
            .connection
            .prepare_cached("SELECT data FROM users WHERE id=?;")?;
        let data = select_stmt.query_row([user.user_id], |r| r.get::<usize, Vec<u8>>(0))?;
        let raw = tl_types::types::User::deserialize(&mut Cursor::from_slice(&data))?;
        Ok(User::from_raw(tl_types::enums::User::User(raw)))
    }

    fn load_group(&self, group: tl_types::types::PeerChat) -> Result<Group> {
        let mut select_stmt = self
            .connection
            .prepare_cached("SELECT data FROM groups WHERE id=?;")?;
        let data = select_stmt.query_row([group.chat_id], |r| r.get::<usize, Vec<u8>>(0))?;
        let raw = tl_types::enums::Chat::deserialize(&mut Cursor::from_slice(&data))?;
        Ok(Group::from_raw(raw))
    }

    fn load_channel(&self, channel: tl_types::types::PeerChannel) -> Result<Channel> {
        let mut select_stmt = self
            .connection
            .prepare_cached("SELECT data FROM channels WHERE id=?;")?;
        let data = select_stmt.query_row([channel.channel_id], |r| r.get::<usize, Vec<u8>>(0))?;
        let raw = tl_types::types::Channel::deserialize(&mut Cursor::from_slice(&data))?;
        Ok(Channel::from_raw(tl_types::enums::Chat::Channel(raw)))
    }

    // Session table has only one row, give it dummy ID=1 for consistency.
    const SESSION_ROW_ID: i32 = 1;

    pub fn load_session(&self) -> Result<Session> {
        let statement = format!(
            "SELECT data FROM session WHERE id={};",
            Self::SESSION_ROW_ID
        );
        let mut cached_statement = self.connection.prepare_cached(&statement)?;
        let data = cached_statement.query_row((), |r| r.get::<usize, Vec<u8>>(0))?;
        let result = Session::load(&data)?;
        Ok(result)
    }

    pub fn save_session(&self, session: &Session) -> Result<()> {
        let statement = format!(
            "INSERT OR REPLACE INTO session(id, data) VALUES ({}, ?);",
            Self::SESSION_ROW_ID
        );
        let mut cached_statement = self.connection.prepare_cached(&statement)?;
        let serialized = session.save();
        cached_statement.execute([serialized])?;
        Ok(())
    }
}

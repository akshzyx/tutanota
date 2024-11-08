use crate::importer::file_reader::import_client::{FileImport, FileIterationError};
use crate::importer::imap_reader::import_client::{ImapImport, ImapIterationError};
use crate::importer::imap_reader::ImapImportConfig;
use crate::importer::importable_mail::ImportableMail;
use crate::tuta::credentials::TutaCredentials;
use napi::bindgen_prelude::Error as NapiError;
use std::sync::Arc;
use tutasdk::crypto::aes::Iv;
use tutasdk::crypto::key::GenericAesKey;
use tutasdk::crypto::randomizer_facade::RandomizerFacade;
use tutasdk::entities::generated::tutanota::{ImportMailData, ImportMailPostIn, ImportMailPostOut};
use tutasdk::login::Credentials;
use tutasdk::net::native_rest_client::NativeRestClient;
use tutasdk::services::generated::tutanota::ImportMailService;
use tutasdk::services::ExtraServiceParams;
use tutasdk::GeneratedId;
use tutasdk::{IdTupleGenerated, LoggedInSdk, Sdk};

pub type NapiTokioMutex<T> = napi::tokio::sync::Mutex<T>;

pub mod file_reader;
pub mod imap_reader;
mod importable_mail;

#[derive(Clone, PartialEq)]
pub enum ImportParams {
    Imap {
        imap_import_config: ImapImportConfig,
    },
    LocalFile {
        file_path: String,
        is_mbox: bool,
    },
}

/// current state of the imap_reader import for this tuta account
/// requires an initialized SDK!
#[cfg_attr(feature = "javascript", napi_derive::napi)]
#[cfg_attr(not(feature = "javascript"), derive(Clone))]
#[derive(PartialEq, Default)]
#[cfg_attr(test, derive(Debug))]
pub enum ImportState {
    #[default]
    NotInitialized,
    Paused,
    Running,
    Postponed,
    Finished,
}

#[cfg_attr(feature = "javascript", napi_derive::napi(object))]
#[derive(PartialEq, Clone, Default)]
#[cfg_attr(test, derive(Debug))]
pub struct ImportStatus {
    pub state: ImportState,
    pub imported_mails: u32,
}

struct Importer {
    status: ImportStatus,
    logged_in_sdk: Arc<LoggedInSdk>,
    target_owner_group: GeneratedId,
    target_mail_folder: IdTupleGenerated,
    import_source: ImportSource,
    randomizer_facade: RandomizerFacade,
}

pub enum ImportSource {
    RemoteImap { imap_import_client: ImapImport },
    LocalFile { fs_email_client: FileImport },
}

/// Wrapper for `Importer` to be used from napi-rs interface
#[cfg_attr(feature = "javascript", napi_derive::napi)]
pub struct ImporterApi {
    inner: Arc<NapiTokioMutex<Importer>>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum IterationError {
    Imap(ImapIterationError),
    File(FileIterationError),
}

impl Importer {
    pub async fn continue_import(&mut self) -> Result<ImportStatus, ()> {
        let mut failed_import_count = 0_u32;
        let mut success_import_count = 0_u32;

        'walk_through_source: loop {
            let next_importable_mail = match &mut self.import_source {
                ImportSource::RemoteImap { imap_import_client } => imap_import_client
                    .fetch_next_mail()
                    .await
                    .map_err(IterationError::Imap),

                ImportSource::LocalFile { fs_email_client } => fs_email_client
                    .get_next_importable_mail()
                    .map_err(IterationError::File),
            };

            let import_res = match next_importable_mail {
                Ok(next_importable_mail) => self.import_one_mail(next_importable_mail).await,

                // source says, all the iteration have ended,
                Err(IterationError::File(FileIterationError::SourceEnd))
                | Err(IterationError::Imap(ImapIterationError::SourceEnd)) => {
                    break 'walk_through_source;
                }

                Err(e) => {
                    panic!("Cannot get next email from source: {e:?}")
                }
            };

            match import_res {
                // this import has been success,
                Ok(_imported_mail_response) => success_import_count += 1,

                Err(_) => {
                    // todo: save the ImportableMail to some fail list,
                    // since, in this iteration the source will not give this mail again,
                    failed_import_count += 1;
                }
            }
        }

        if failed_import_count > 0 {
            // some mail failed to import:
            self.status = ImportStatus {
                state: ImportState::Postponed,
                imported_mails: success_import_count,
            };
        } else {
            // nothing failed,
            self.status = ImportStatus {
                state: ImportState::Finished,
                imported_mails: success_import_count,
            };
        };

        Ok(self.status.clone())
    }

    /// once we get the ImportableMail from either of source,
    /// continue to the uploading counterpart
    async fn import_one_mail(
        &self,
        importable_mail: ImportableMail,
    ) -> Result<ImportMailPostOut, ()> {
        let new_aes_256_key = GenericAesKey::from_bytes(
            self.randomizer_facade
                .generate_random_array::<{ tutasdk::crypto::aes::AES_256_KEY_SIZE }>()
                .as_slice(),
        )
            .unwrap();
        let mail_group_key = self
            .logged_in_sdk
            .get_current_sym_group_key(&self.target_owner_group)
            .await
            .map_err(|e| ())?;
        let owner_enc_session_key =
            mail_group_key.encrypt_key(&new_aes_256_key, Iv::generate(&self.randomizer_facade));

        let import_mail_data = ImportMailData::from(importable_mail);
        let import_mail_post_in = ImportMailPostIn {
            ownerEncSessionKey: owner_enc_session_key.object,
            ownerGroup: self.target_owner_group.clone(),
            ownerKeyVersion: owner_enc_session_key.version,
            imports: vec![import_mail_data],
            targetMailFolder: self.target_mail_folder.clone(),
            _format: 0,
            _errors: None,
            _finalIvs: Default::default(),
        };

        let service_params = ExtraServiceParams {
            session_key: Some(new_aes_256_key),
            ..Default::default()
        };

        let import_mail_post_out = self
            .logged_in_sdk
            .get_service_executor()
            .post::<ImportMailService>(import_mail_post_in, service_params)
            .await
            .expect("Cannot execute ImportMailService");

        Ok(import_mail_post_out)
    }
}

impl ImporterApi {
    pub fn new(
        logged_in_sdk: Arc<LoggedInSdk>,
        target_owner_group: GeneratedId,
        target_mail_folder: IdTupleGenerated,
        import_source: ImportSource,
    ) -> Self {
        let import_inner = Importer {
            logged_in_sdk,
            target_owner_group,
            target_mail_folder,
            import_source,
            status: ImportStatus::default(),
            randomizer_facade: RandomizerFacade::from_core(rand::rngs::OsRng),
        };
        Self {
            inner: Arc::new(NapiTokioMutex::new(import_inner)),
        }
    }

    pub async fn continue_import_inner(&mut self) -> Result<ImportStatus, ()> {
        self.inner.lock().await.continue_import().await
    }

    pub async fn delete_import_inner(&mut self) -> Result<ImportStatus, ()> {
        todo!()
    }

    pub async fn pause_import_inner(&mut self) -> Result<ImportStatus, ()> {
        todo!()
    }

    pub async fn create_file_importer_inner(
        tuta_credentials: TutaCredentials,
        target_owner_group: String,
        target_mail_folder: (String, String),
        source_paths: Vec<String>,
    ) -> napi::Result<ImporterApi> {
        let logged_in_sdk_future = Self::create_sdk(tuta_credentials);

        let fs_email_client = FileImport::new(source_paths)
            .map_err(|e| NapiError::from_reason("Cannot create file import"))?;
        let import_source = ImportSource::LocalFile { fs_email_client };
        let logged_in_sdk = logged_in_sdk_future
            .await
            .map_err(|e| NapiError::from_reason("Cannot create logged in sdk"))?;

        Ok(ImporterApi::new(
            logged_in_sdk,
            GeneratedId(target_owner_group),
            IdTupleGenerated::new(
                GeneratedId(target_mail_folder.0),
                GeneratedId(target_mail_folder.1),
            ),
            import_source,
        ))
    }

    async fn create_sdk(tuta_credentials: TutaCredentials) -> Result<Arc<LoggedInSdk>, String> {
        let rest_client = Arc::new(
            NativeRestClient::try_new()
                .map_err(|e| format!("Cannot build native rest client: {e}"))?,
        );

        let logged_in_sdk = {
            let sdk = Sdk::new(tuta_credentials.api_url.clone(), rest_client);

            let sdk_credentials: Credentials = tuta_credentials
                .clone()
                .try_into()
                .map_err(|_| "Cannot convert to valid credentials".to_string())?;
            sdk.login(sdk_credentials)
                .await
                .map_err(|e| format!("Cannot login to sdk. Error: {:?}", e))?
        };

        Ok(logged_in_sdk)
    }
}

// Wrapper for napi
#[cfg(feature = "javascript")]
#[napi_derive::napi]
impl ImporterApi {
    // once Self::continue_import return custom error,
    // do the error conversion here, or in <From> trait
    fn error_conversion<E>(err: E) -> napi::Error {
        todo!()
    }

    #[napi]
    pub async unsafe fn continue_import(&mut self) -> napi::Result<ImportStatus> {
        self.continue_import_inner()
            .await
            .map_err(Self::error_conversion)
    }

    #[napi]
    pub async unsafe fn delete_import(&mut self) -> napi::Result<ImportStatus> {
        self.delete_import_inner()
            .await
            .map_err(Self::error_conversion)
    }

    #[napi]
    pub async unsafe fn pause_import(&mut self) -> napi::Result<ImportStatus> {
        self.pause_import_inner()
            .await
            .map_err(Self::error_conversion)
    }

    #[napi]
    pub async fn create_file_importer(
        tuta_credentials: TutaCredentials,
        target_owner_group: String,
        target_mail_folder: (String, String),
        source_paths: Vec<String>,
    ) -> napi::Result<ImporterApi> {
        Self::create_file_importer_inner(
            tuta_credentials,
            target_owner_group,
            target_mail_folder,
            source_paths,
        )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::imap_reader::{ImapCredentials, LoginMechanism};
    use crate::tuta_imap::testing::GreenMailTestServer;
    use mail_builder::MessageBuilder;
    use tutasdk::entities::generated::tutanota::MailFolder;
    use tutasdk::folder_system::MailSetKind;
    use tutasdk::net::native_rest_client::NativeRestClient;
    use tutasdk::Sdk;

    fn sample_email(subject: String) -> String {
        let email = MessageBuilder::new()
            .from(("Matthias", "map@example.org"))
            .to(("Johannes", "jhm@example.org"))
            .subject(subject)
            .text_body("Hello tutao! this is the first step to have email import.Want to see html 😀?<p style='color:red'>red</p>")
            .write_to_string()
            .unwrap();
        email
    }

    async fn get_test_import_folder_id(
        logged_in_sdk: &Arc<LoggedInSdk>,
        kind: MailSetKind,
    ) -> MailFolder {
        let mail_facade = logged_in_sdk.mail_facade();
        let mailbox = mail_facade.load_user_mailbox().await.unwrap();
        let folders = mail_facade
            .load_folders_for_mailbox(&mailbox)
            .await
            .unwrap();
        folders
            .system_folder_by_type(kind)
            .expect("inbox should exist")
            .clone()
    }

    async fn init_imap_importer() -> (Importer, GreenMailTestServer) {
        let importer_mail_address = "map-free@tutanota.de".to_string();
        let logged_in_sdk = Sdk::new(
            "http://localhost:9000".to_string(),
            Arc::new(NativeRestClient::try_new().unwrap()),
        )
            .create_session(importer_mail_address.as_str(), "map")
            .await
            .unwrap();
        let greenmail = GreenMailTestServer::new();
        let imap_import_config = ImapImportConfig {
            root_import_mail_folder_name: "/".to_string(),
            credentials: ImapCredentials {
                host: "127.0.0.1".to_string(),
                port: greenmail.imaps_port.try_into().unwrap(),
                login_mechanism: LoginMechanism::Plain {
                    username: "sug@example.org".to_string(),
                    password: "sug".to_string(),
                },
            },
        };

        let import_source = ImportSource::RemoteImap {
            imap_import_client: ImapImport::new(imap_import_config),
        };
        let randomizer_facade = RandomizerFacade::from_core(rand::rngs::OsRng);
        let target_mail_folder = get_test_import_folder_id(&logged_in_sdk, MailSetKind::Archive)
            .await
            ._id
            .unwrap();

        let target_owner_group = logged_in_sdk
            .mail_facade()
            .get_group_id_for_mail_address(importer_mail_address.as_str())
            .await
            .unwrap();

        let importer = Importer {
            target_owner_group,
            target_mail_folder,
            logged_in_sdk,
            import_source,
            randomizer_facade,
            status: ImportStatus::default(),
        };

        (importer, greenmail)
    }

    pub async fn init_file_importer(source_paths: Vec<String>) -> Importer {
        let logged_in_sdk = Sdk::new(
            "http://localhost:9000".to_string(),
            Arc::new(NativeRestClient::try_new().unwrap()),
        )
            .create_session("map-free@tutanota.de", "map")
            .await
            .unwrap();

        let import_source = ImportSource::LocalFile {
            fs_email_client: FileImport::new(source_paths).unwrap(),
        };
        let randomizer_facade = RandomizerFacade::from_core(rand::rngs::OsRng);
        let target_mail_folder = get_test_import_folder_id(&logged_in_sdk, MailSetKind::Archive)
            .await
            ._id
            .unwrap();

        let target_owner_group = logged_in_sdk
            .mail_facade()
            .get_group_id_for_mail_address("map-free@tutanota.de")
            .await
            .unwrap();

        Importer {
            status: ImportStatus::default(),
            target_owner_group,
            target_mail_folder,
            logged_in_sdk,
            import_source,
            randomizer_facade,
        }
    }

    #[tokio::test]
    pub async fn import_multiple_from_imap_default_folder() {
        let (mut importer, greenmail) = init_imap_importer().await;

        let email_first = sample_email("Hello from imap 😀! -- Список.doc".to_string());
        let email_second = sample_email("Second time: hello".to_string());
        greenmail.store_mail("sug@example.org", email_first.as_str());
        greenmail.store_mail("sug@example.org", email_second.as_str());

        let import_res = importer.continue_import().await.map_err(|_| ());
        assert_eq!(
            Ok(ImportStatus {
                state: ImportState::Finished,
                imported_mails: 2,
            }),
            import_res
        );
    }

    #[tokio::test]
    pub async fn import_single_from_imap_default_folder() {
        let (mut importer, greenmail) = init_imap_importer().await;

        let email = sample_email("Single email".to_string());
        greenmail.store_mail("sug@example.org", email.as_str());

        let import_res = importer.continue_import().await.map_err(|_| ());
        assert_eq!(
            Ok(ImportStatus {
                state: ImportState::Finished,
                imported_mails: 1,
            }),
            import_res
        );
    }

    #[tokio::test]
    async fn can_import_single_eml_file() {
        let mut importer = init_file_importer(vec!["./test/sample.eml".to_string()]).await;

        let import_res = importer.continue_import().await.map_err(|_| ());
        assert_eq!(
            Ok(ImportStatus {
                state: ImportState::Finished,
                imported_mails: 1,
            }),
            import_res
        );
    }
}

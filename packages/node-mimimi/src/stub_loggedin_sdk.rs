use std::sync::Arc;
use tutasdk::crypto::key::{GenericAesKey, VersionedAesKey};
use tutasdk::entities::generated::sys::BlobReferenceTokenWrapper;
use tutasdk::services::service_executor::ResolvingServiceExecutor;
use tutasdk::services::{ExtraServiceParams, PostService};
use tutasdk::tutanota_constants::ArchiveDataType;
use tutasdk::LoggedInSdk as RealLoggedInSdk;
use tutasdk::{ApiCallError, GeneratedId};

#[async_trait::async_trait]
// todo: think about restructuring to expose real LoggedInSdk hierarchy
pub trait LoggedInSdk: Sync + Send + Sized + 'static {
	async fn get_current_sym_group_key(
		&self,
		user_group_id: &GeneratedId,
	) -> Result<VersionedAesKey, ApiCallError>;

	async fn encrypt_and_upload(
		&self,
		archive_data_type: ArchiveDataType,
		owner_group_id: &GeneratedId,
		session_key: &GenericAesKey,
		blob_data: Vec<u8>,
	) -> Result<Vec<BlobReferenceTokenWrapper>, ApiCallError>;

	async fn post_service_executor<Service: PostService>(
		&self,
		data: Service::Input,
		params: ExtraServiceParams,
	) -> Result<Service::Output, ApiCallError>;

	async fn get_group_id_for_mail_address(
		&self,
		mail_address: &str,
	) -> Result<GeneratedId, ApiCallError>;
}

pub struct StubLoggedInSdk {
	pub get_current_sym_group_key: Box<
		dyn Send
			+ Sync
			+ Fn(&StubLoggedInSdk, &GeneratedId) -> Result<VersionedAesKey, ApiCallError>,
	>,

	pub encrypt_and_upload: Box<
		dyn Send
			+ Sync
			+ Fn(
				&StubLoggedInSdk,
				ArchiveDataType,
				&GeneratedId,
				&GenericAesKey,
				Vec<u8>,
			) -> Result<Vec<BlobReferenceTokenWrapper>, ApiCallError>,
	>,

	pub get_service_executor:
		Box<dyn Send + Sync + Fn(&StubLoggedInSdk) -> &Arc<ResolvingServiceExecutor>>,

	pub get_group_id_for_mail_address:
		Box<dyn Send + Sync + Fn(&StubLoggedInSdk, &str) -> Result<GeneratedId, ApiCallError>>,
}

#[async_trait::async_trait]
impl LoggedInSdk for StubLoggedInSdk {
	async fn get_current_sym_group_key(
		&self,
		user_group_id: &GeneratedId,
	) -> Result<VersionedAesKey, ApiCallError> {
		(*self.get_current_sym_group_key)(self, user_group_id)
	}

	async fn post_service_executor<Service: PostService>(
		&self,
		data: Service::Input,
		params: ExtraServiceParams,
	) -> Result<Service::Output, ApiCallError> {
		Ok(todo!())
	}

	async fn encrypt_and_upload(
		&self,
		archive_data_type: ArchiveDataType,
		owner_group_id: &GeneratedId,
		session_key: &GenericAesKey,
		blob_data: Vec<u8>,
	) -> Result<Vec<BlobReferenceTokenWrapper>, ApiCallError> {
		(*self.encrypt_and_upload)(
			self,
			archive_data_type,
			owner_group_id,
			session_key,
			blob_data,
		)
	}

	async fn get_group_id_for_mail_address(
		&self,
		mail_address: &str,
	) -> Result<GeneratedId, ApiCallError> {
		(*self.get_group_id_for_mail_address)(self, mail_address)
	}
}

impl Default for StubLoggedInSdk {
	fn default() -> Self {
		StubLoggedInSdk {
			get_current_sym_group_key: Box::new(|_, _| {
				Ok(VersionedAesKey {
					object: GenericAesKey::from_bytes(&[0; 32]).unwrap(),
					version: 0,
				})
			}),
			encrypt_and_upload: Box::new(|_, _, _, _, _| {
				Ok(vec![
					BlobReferenceTokenWrapper {
						_id: None,
						blobReferenceToken: "first blobReferenceToken".to_string(),
					},
					BlobReferenceTokenWrapper {
						_id: None,
						blobReferenceToken: "second blobReferenceToken".to_string(),
					},
				])
			}),
			get_service_executor: Box::new(|_| todo!()),
			get_group_id_for_mail_address: Box::new(|_, _| {
				Ok(GeneratedId("generatedId".to_string()))
			}),
		}
	}
}

#[async_trait::async_trait]
impl LoggedInSdk for RealLoggedInSdk {
	#[inline(always)]
	async fn get_current_sym_group_key(
		&self,
		user_group_id: &GeneratedId,
	) -> Result<VersionedAesKey, ApiCallError> {
		self.get_current_sym_group_key(user_group_id).await
	}

	async fn post_service_executor<Service: PostService>(
		&self,
		data: Service::Input,
		params: ExtraServiceParams,
	) {
		self.get_service_executor().post::<Service>(data, params)
	}

	#[inline(always)]
	async fn encrypt_and_upload(
		&self,
		archive_data_type: ArchiveDataType,
		owner_group_id: &GeneratedId,
		session_key: &GenericAesKey,
		blob_data: Vec<u8>,
	) -> Result<Vec<BlobReferenceTokenWrapper>, ApiCallError> {
		self.blob_facade()
			.encrypt_and_upload(archive_data_type, owner_group_id, &session_key, blob_data)
			.await
	}

	async fn get_group_id_for_mail_address(
		&self,
		mail_address: &str,
	) -> Result<GeneratedId, ApiCallError> {
		self.mail_facade()
			.get_group_id_for_mail_address(mail_address)
			.await
	}
}

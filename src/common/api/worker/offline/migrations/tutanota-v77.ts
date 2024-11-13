import { OfflineMigration } from "../OfflineStorageMigrator.js"
import { OfflineStorage } from "../OfflineStorage.js"
import { addValue, migrateAllElements } from "../StandardMigrations"
import { MailBoxTypeRef } from "../../../entities/tutanota/TypeRefs"

export const tutanotaV77: OfflineMigration = {
	app: "tutanota",
	version: 77,
	async migrate(storage: OfflineStorage) {
		// TODO think about offline migration for Mailbox#importedAttachments
		await migrateAllElements(MailBoxTypeRef, storage, [addValue("importedAttachments", "")])
	},
}

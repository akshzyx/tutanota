import { OfflineMigration } from "../OfflineStorageMigrator.js"
import { OfflineStorage, TableDefinitions } from "../OfflineStorage.js"
import { SqlCipherFacade } from "../../../../native/common/generatedipc/SqlCipherFacade"

export const offline2: OfflineMigration = {
	app: "offline",
	version: 2,
	async migrate(storage: OfflineStorage, sqlCipherFacade: SqlCipherFacade) {
		console.log("migrating to offline v2")
		await sqlCipherFacade.run(`CREATE TABLE IF NOT EXISTS verification_pool (${TableDefinitions.verification_pool})`, [])
	},
}

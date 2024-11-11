import { assertNotNull } from "@tutao/tutanota-utils"

export async function requestFromWebsite(path: string): Promise<Response> {
	const url = new URL(path, assertNotNull(env.websiteUrl))
	return fetch(url.href)
}

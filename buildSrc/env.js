/** @param {{staticUrl: string | null, websiteUrl: string | null, version: string, mode: EnvMode | null, dist: boolean, domainConfigs: DomainConfigMap}} params
 *  @return {env}
 */
export function create(params) {
	const { staticUrl, websiteUrl, version, mode, dist, domainConfigs } = params

	if (version == null || mode == null || dist == null) {
		throw new Error(`Invalid env parameters: ${JSON.stringify(params)}`)
	}
	return {
		staticUrl: staticUrl?.toString(),
		websiteUrl: websiteUrl?.toString(),
		versionNumber: version,
		dist,
		mode: mode ?? "Browser",
		timeout: 20000,
		domainConfigs,
		platformId: null,
	}
}

/** @param {env} env */
export function preludeEnvPlugin(env) {
	return {
		name: "prelude-env",
		banner() {
			return `globalThis.env = ${JSON.stringify(env, null, 2)};`
		},
	}
}

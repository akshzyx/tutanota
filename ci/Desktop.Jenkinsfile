pipeline {
    environment {
    	// on m1 macs, this is a symlink that must be updated. see wiki.
        VERSION = sh(returnStdout: true, script: "${env.NODE_PATH}/node -p -e \"require('./package.json').version\" | tr -d \"\n\"")
        TMPDIR='/tmp'
    }
    options {
		preserveStashes()
	}

	parameters {
		booleanParam(
			name: 'RELEASE',
			defaultValue: false,
			description: "Prepare a release version (doesn't publish to production, this is done manually)"
		)
        booleanParam(
			name: 'WINDOWS',
			defaultValue: false,
			description: "Build Windows client."
		)
        booleanParam(
			name: 'MAC',
			defaultValue: false,
			description: "Build Mac client."
		)
        booleanParam(
			name: 'LINUX',
			defaultValue: false,
			description: "Build Linux client."
		)
	}

	agent {
		label 'master'
	}

    stages {
        stage("Checking params") {
            steps {
                script{
                    if(!params.WINDOWS && !params.MAC && !params.LINUX) {
                        currentBuild.result = 'ABORTED'
                        error('No artifacts were selected.')
                    }
                }
                echo "Params OKAY"
            }
        }
		stage('Check Github') {
			steps {
				script {
					def util = load "ci/jenkins-lib/util.groovy"
					util.checkGithub()
				}
			}
		}
		stage('Build dependencies') {
			parallel {
				stage('Build webapp') {
					agent {
						dockerfile {
							filename 'linux-build.dockerfile'
							label 'master'
							dir 'ci/containers'
							additionalBuildArgs "--format docker"
							args '--network host'
						} // docker
					} // agent
					steps {
						sh 'npm ci'
						sh 'npm run build-packages'
						sh 'node webapp.js release'

						// excluding web-specific and mobile specific parts which we don't need in desktop
						stash includes: 'build/**', excludes: '**/braintree.html, **/index.html, **/app.html, **/desktop.html, **/index-index.js, **/index-app.js, **/index-desktop.js, **/sw.js', name: 'web_base'
					}
				}

				stage('Native modules') {
					agent {
						label 'win-native'
					}
					steps {
						bat "npm ci"

						bat "node buildSrc\\getNativeLibrary.js better-sqlite3 --copy-target better_sqlite3 --force-rebuild --root-dir ${WORKSPACE}"
						stash includes: 'native-cache/**/*', name: 'native_modules'
					}
				}
			}
		}

		stage('Build desktop clients') {
			parallel {
				stage('Windows') {
				    when {
				        expression { return params.WINDOWS }
				    }
					environment {
						PATH = "${env.NODE_PATH}:${env.PATH}"
					}
					agent {
						label 'win-cross-compile'
					}
					steps {
						initBuildArea()

						// nativeLibraryProvider.js placed the built native modules in the correct location (native-cache)
						// so they will be picked up by our rollup plugin
						unstash 'native_modules'

						// add DEBUG for electron-builder because it tends to not let us know about things failing
						withCredentials([string(credentialsId: 'YUBI_28989236_PIN', variable: 'PW')]) {
							sh '''
							export YUBI_PIN=${PW};
							DEBUG=electron-builder node desktop --existing --platform win '''
						}

						dir('artifacts') {
							stash includes: 'desktop-test/*', name:'win_installer_test'
							stash includes: 'desktop/*', name:'win_installer'
						}
					}
                }

                stage('Mac') {
				    when {
				        expression { return params.MAC }
				    }
					environment {
						PATH = "${env.NODE_MAC_PATH}:${env.PATH}"
					}
					agent {
						label 'mac-m1'
					}
					steps {
						initBuildArea()

						withCredentials([
								usernamePassword(credentialsId: 'APP_NOTARIZE_CREDS', usernameVariable: 'APPLEIDVAR', passwordVariable: 'APPLEIDPASSVAR'),
								string(credentialsId: 'fastlane-keychain-password', variable: 'FASTLANE_KEYCHAIN_PASSWORD'),
								string(credentialsId: 'team-id', variable: 'APPLETEAMIDVAR'),
						]) {
							sh 'security unlock-keychain -p $FASTLANE_KEYCHAIN_PASSWORD'
							script {
								def stage = params.RELEASE ? 'release' : 'prod'
								sh '''
								export APPLEID=${APPLEIDVAR};
								export APPLEIDPASS=${APPLEIDPASSVAR};
								export APPLETEAMID=${APPLETEAMIDVAR};
								node desktop --existing --architecture universal --platform mac ''' + "${stage}"
								dir('artifacts') {
									if (params.RELEASE) {
										stash includes: 'desktop-test/*', name:'mac_installer_test'
									}
									stash includes: 'desktop/*', name:'mac_installer'
								}
							}
						}
					}
				}

				stage('Linux') {
				    when {
				        expression { return params.LINUX }
				    }
					agent {
						dockerfile {
							filename 'linux-build.dockerfile'
							label 'master'
							dir 'ci/containers'
							additionalBuildArgs "--format docker"
							args '--network host'
						} // docker
					}
					steps {
						initBuildArea()

						sh 'node desktop --existing --platform linux'

						dir('artifacts') {
							stash includes: 'desktop-test/*', name:'linux_installer_test'
							stash includes: 'desktop/*', name:'linux_installer'
						}
					}
				}
			}
		}

		stage('Preparation for sign clients and upload to Nexus') {
			when { expression { return params.RELEASE } }
			agent {
				label 'master'
			}
			steps {
				script {
					def devicePath =  sh(script: 'lsusb | grep Nitro | sed -nr \'s|Bus (.*) Device ([^:]*):.*|/dev/bus/usb/\\1/\\2|p\'', returnStdout: true).trim()
					env.DEVICE_PATH = devicePath
				}
			}
		}
		stage('Sign clients and upload to Nexus') {
			when { expression { return params.RELEASE } }
			agent {
				dockerfile {
					filename 'linux-build.dockerfile'
					label 'master'
					dir 'ci/containers'
					additionalBuildArgs '--format docker'
					args "--network host -v /run:/run:rw,z -v /opt/repository:/opt/repository:rw,z --device=${env.DEVICE_PATH}"
				} // docker
		    }
		    environment {
                PATH = "${env.NODE_PATH}:${env.PATH}"
				UTIL = load "ci/jenkins-lib/util.groovy"
            }
		    stages {
                // #FIXME rename
                stage('build-packages') {
                    steps {
                        sh 'npm ci'
                        sh 'npm run build-packages'
                        sh 'rm -rf ./build/*'
                    }
                }
                stage('Sign and upload') {
                    parallel {
                        stage('Window') {
                            when {
                                expression { return params.WINDOWS }
                            }
                            steps {
                                dir('build') {
                                    unstash 'win_installer'
                                    unstash 'win_installer_test'
                                }

                                withCredentials([string(credentialsId: 'HSM_USER_PIN', variable: 'PW')]) {
                                    sh '''export HSM_USER_PIN=${PW}; node buildSrc/signDesktopClients.js'''
                                }


                                script {
                                    if (!fileExists("build/desktop/tutanota-desktop-win.exe")) {
                                        currentBuild.result = 'ABORTED'
                                        error("Unable to find file windows client")
                                    }
                                }

        //                             util.publishToNexus(
        //                                     groupId: "app",
        //                                     artifactId: "desktop-win-test",
        //                                     version: "${VERSION}",
        //                                     assetFilePath: "${WORKSPACE}/build/desktop-test/tutanota-desktop-test-win.exe",
        //                                     fileExtension: 'exe'
        //                             )
        //                             util.publishToNexus(
        //                                     groupId: "app",
        //                                     artifactId: "desktop-win",
        //                                     version: "${VERSION}",
        //                                     assetFilePath: "${WORKSPACE}/build/desktop/tutanota-desktop-win.exe",
        //                                     fileExtension: 'exe'
        //                             )
                            }
                        }
                        stage('Mac') {
                            when {
                                expression { return params.MAC }
                            }
                            steps {
                                 dir('build') {
                                     unstash 'mac_installer'
                                     unstash 'mac_installer_test'
                                 }

                                withCredentials([string(credentialsId: 'HSM_USER_PIN', variable: 'PW')]) {
                                    sh '''export HSM_USER_PIN=${PW}; node buildSrc/signDesktopClients.js'''
                                }


                                script {
                                    if (!fileExists("build/desktop/tutanota-desktop-mac.dmg")) {
                                        currentBuild.result = 'ABORTED'
                                        error("Unable to find file mac client")
                                    }
                                }

        //                             util.publishToNexus(
        //                                     groupId: "app",
        //                                     artifactId: "desktop-mac-test",
        //                                     version: "${VERSION}",
        //                                     assetFilePath: "${WORKSPACE}/build/desktop-test/tutanota-desktop-test-mac.dmg",
        //                                     fileExtension: 'dmg'
        //                             )
        //                             util.publishToNexus(
        //                                     groupId: "app",
        //                                     artifactId: "desktop-mac",
        //                                     version: "${VERSION}",
        //                                     assetFilePath: "${WORKSPACE}/build/desktop/tutanota-desktop-mac.dmg",
        //                                     fileExtension: 'dmg'
        //                             )
                            }
                        }
                        stage('Linux') {
                            when {
                                expression { return params.LINUX }
                            }
                            steps {
                                 dir('build') {
                                     unstash 'linux_installer'
                                     unstash 'linux_installer_test'
                                 }

                                withCredentials([string(credentialsId: 'HSM_USER_PIN', variable: 'PW')]) {
                                    sh '''export HSM_USER_PIN=${PW}; node buildSrc/signDesktopClients.js'''
                                }


                                script {
                                    if (!fileExists("build/desktop/tutanota-desktop-linux.AppImage")) {
                                        currentBuild.result = 'ABORTED'
                                        error("Unable to find file linux client")
                                    }
                                }

        //                             util.publishToNexus(
        //                                     groupId: "app",
        //                                     artifactId: "desktop-linux-test",
        //                                     version: "${VERSION}",
        //                                     assetFilePath: "${WORKSPACE}/build/desktop-test/tutanota-desktop-test-linux.AppImage",
        //                                     fileExtension: 'AppImage'
        //                             )
        //                             util.publishToNexus(
        //                                     groupId: "app",
        //                                     artifactId: "desktop-linux",
        //                                     version: "${VERSION}",
        //                                     assetFilePath: "${WORKSPACE}/build/desktop/tutanota-desktop-linux.AppImage",
        //                                     fileExtension: 'AppImage'
        //                             )
                            }
                        }
                    } // parallel
                } // Sign and upload
            } // stages
		} // stage sign clients and upload to Nexus
	} // stages
} // pipeline

void initBuildArea() {
	sh 'node -v'
	sh 'npm -v'
    sh 'npm ci'
    sh 'npm run build-packages'
    sh 'rm -rf ./build/*'
    sh 'rm -rf ./native-cache/*'
    unstash 'web_base'
}

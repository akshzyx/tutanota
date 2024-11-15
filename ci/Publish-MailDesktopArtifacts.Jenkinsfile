pipeline {
    environment {
    	// on m1 macs, this is a symlink that must be updated. see wiki.
        VERSION = sh(returnStdout: true, script: "${env.NODE_PATH}/node -p -e \"require('./package.json').version\" | tr -d \"\n\"")
        TMPDIR='/tmp'
    }

	parameters {
		persistentText(
			name: "releaseNotes",
			defaultValue: "",
			description: "release notes for this build"
		 )
	}

	agent {
        label 'master'
	}

    stages {
		stage('Check Github') {
			steps {
				script {
					def util = load "ci/jenkins-lib/util.groovy"
					util.checkGithub()
				}
			}
		}
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
        stage('Preparation for build deb and publish') {
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
		stage('Build deb and publish') {
			agent {
				dockerfile {
					filename 'linux-build.dockerfile'
					label 'master'
					dir 'ci/containers'
					additionalBuildArgs '--format docker'
					args "--network host -v /run:/run:rw,z -v /opt/repository:/opt/repository:rw,z --device=${env.DEVICE_PATH}"
				} // docker
		    }
		    environment { PATH = "${env.NODE_PATH}:${env.PATH}" }
			steps {
				script { // create release draft
					def desktopLinux = "build/desktop/tutanota-desktop-linux.AppImage"
					def desktopLinuxTest = "build/desktop-test/tutanota-desktop-test-linux.AppImage"
					def desktopWin = "build/desktop/tutanota-desktop-win.exe"
					def desktopMac = "build/desktop/tutanota-desktop-mac.dmg"

                    def util = load "ci/jenkins-lib/util.groovy"

                    util.downloadFromNexus(	groupId: "app",
                                            artifactId: "desktop-win",
                                            version: "${VERSION}",
                                            outFile: "${WORKSPACE}/${desktopWin}",
                                            fileExtension: 'exe')
                    if (!fileExists("${desktopWin}")) {
                        currentBuild.result = 'ABORTED'
                        error("Unable to find file ${desktopWin}")
                    }

                    util.downloadFromNexus(	groupId: "app",
                                            artifactId: "desktop-mac",
                                            version: "${VERSION}",
                                            outFile: "${WORKSPACE}/${desktopMac}",
                                            fileExtension: 'dmg')
                    if (!fileExists("${desktopMac}")) {
                        currentBuild.result = 'ABORTED'
                        error("Unable to find file ${desktopMac}")
                    }

                    util.downloadFromNexus(	groupId: "app",
                                            artifactId: "desktop-linux",
                                            version: "${VERSION}",
                                            outFile: "${WORKSPACE}/${desktopLinux}",
                                            fileExtension: 'AppImage')
                    if (!fileExists("${desktopLinux}")) {
                        currentBuild.result = 'ABORTED'
                        error("Unable to find file ${desktopLinux}")
                    }

                    util.downloadFromNexus(	groupId: "app",
                                            artifactId: "desktop-linux-test",
                                            version: "${VERSION}",
                                            outFile: "${WORKSPACE}/${desktopLinuxTest}",
                                            fileExtension: 'AppImage')
                    if (!fileExists("${desktopLinuxTest}")) {
                        currentBuild.result = 'ABORTED'
                        error("Unable to find file ${desktopLinuxTest}")
                    }

					writeFile file: "notes.txt", text: params.releaseNotes
					catchError(stageResult: 'UNSTABLE', buildResult: 'SUCCESS', message: 'Failed to create github release page for desktop') {
						withCredentials([string(credentialsId: 'github-access-token', variable: 'GITHUB_TOKEN')]) {
							sh """node buildSrc/createReleaseDraft.js --name '${VERSION} (Desktop)' \
																   --tag 'tutanota-desktop-release-${VERSION}' \
																   --uploadFile '${WORKSPACE}/${desktopLinux}' \
																   --uploadFile '${WORKSPACE}/${desktopWin}' \
																   --uploadFile '${WORKSPACE}/${desktopMac}' \
																   --notes notes.txt"""
						} // withCredentials
					} // catchError
					sh "rm notes.txt"
				} // script release draft

                sh 'npm ci'
                sh 'npm run build-packages'
                unstash 'web_base'
                sh 'node buildSrc/publish.js desktop'
                sh 'rm -rf ./build/*'
			} // steps
		} // stage build deb & publish
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
pipeline {
    agent any

    parameters {
        text(name: 'settings', description: 'Telamon configuration file')
        string(name: 'kernel', description: 'Kernel to use', defaultValue: 'matmul')
        choice(choices: 'cpu\ngpu', name: 'device', description: 'Device to run the kernel on', defaultValue: 'gpu')
    }

    stages {
        stage('Build') {
            steps {
                dir('telamon-py') {
                    withEnv(['TELAMON_CUDA_ENABLE=1']) {
                        withPythonEnv('System-CPython-3.4') {
                            pysh 'python setup.py bdist_wheel'
                        }
                    }
                }
            }

            post {
                success {
                    dir('telamon-py/dist') {
                        archiveArtifacts(artifacts: '*.whl', fingerprint: true)
                    }

                    stash name: 'telamon-py', includes: 'telamon-py/dist/*.whl'
                }
            }
        }

        stage('Run') {
            steps {
                unstash 'telamon-py'

                withPythonEnv('System-CPython-3.4') {
                    pysh 'python -m pip install -U toml telamon-py/dist/telamon-0.0.1-py2.py3-none-linux_x86_64.whl'

                    sh 'mkdir -p output'
                    dir('output') {
                        sh "echo '${params.settings}' > Settings.toml"
                        pysh "RUST_LOG=telamon::explorer=warn python ../telamon-py/examples/search.py --device ${params.device} --kernel ${params.kernel} --config Settings.toml"
                    }
                }
            }

            post {
                success {
                    archiveArtifacts(artifacts: 'output/*')
                }
            }
        }
    }
}

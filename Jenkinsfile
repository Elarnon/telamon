pipeline {
    agent any

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
                        pysh 'RUST_LOG=telamon::explorer=warn python ../telamon-py/examples/search.py'
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

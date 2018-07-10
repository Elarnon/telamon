pipeline {
    agent { label 'ficus' }

    stages {
        stage('Build') {
            steps {
                sh 'mkdir -p output && cp Settings.toml output'
                dir('output') {
                    sh 'RUST_LOG=telamon::explorer=warn cargo run --features cuda --manifest-path ../kernels/Cargo.toml --release --bin search -- --device cuda'
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

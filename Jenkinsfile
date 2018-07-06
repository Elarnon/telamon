pipeline {
    agent { label 'ficus' }

    stages {
        stage('Build') {
            steps {
                archiveArtifacts(artifacts: 'Cargo.toml')
            }
        }
    }
}

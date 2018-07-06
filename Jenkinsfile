pipeline {
    agent { label 'ficus' }

    stages {
        stage('Build') {
            ssh-agent(credentials: ['ficus']) {
                sh 'scp Cargo.toml localhost:/home/elarnon/experiments/'
            }
            steps {
            }
        }
    }
}

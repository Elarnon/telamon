pipeline {
    agent { label 'ficus' }

    stages {
        stage('Build') {
            steps {
                sh 'echo test'
            }

            ssh-agent(credentials: ['ficus']) {
                sh 'scp Cargo.toml localhost:/home/elarnon/experiments/'
            }
        }
    }
}

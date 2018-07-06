pipeline {
    agent { label 'ficus' }

    stages {
        stage('Build') {
            steps {
                ssh-agent(credentials: ['ficus']) {
                    sh 'scp Cargo.toml localhost:/home/elarnon/experiments/'
                }
            }
        }
    }
}

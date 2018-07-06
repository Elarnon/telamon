pipeline {
    agent { label 'ficus' }

    stages {
        ssh-agent(credentials: ['ficus']) {
            stage('Build') {
                steps {
                    sh 'scp Cargo.toml localhost:/home/elarnon/experiments/'
                }
            }
        }
    }
}

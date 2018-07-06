pipeline {
    agent { label 'ficus' }

    stages {
        stage('Build') {
            steps {
                withCredentials([sshUserPrivateKey(credentialsId: "elarnon", keyFileVariable: 'keyfile')]) {
                    sh 'scp -i ${keyfile} Cargo.toml localhost:/home/elarnon/experiments/'
                }
            }
        }
    }
}

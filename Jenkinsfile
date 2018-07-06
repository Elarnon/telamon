pipeline {
    agent { label 'ficus' }

    stages {
        stage('Build') {
            withCredentials([sshUserPrivateKey(credentialsId: "ficus", keyFileVariable: 'keyfile')]) {
                steps {
                    sh 'scp -i ${keyfile} Cargo.toml localhost:/home/elarnon/experiments/'
                }
            }
        }
    }
}

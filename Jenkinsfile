pipeline {
    agent { label 'ficus' }

    stages {
        withCredentials([sshUserPrivateKey(credentialsId: "ficus", keyFileVariable: 'keyfile')]) {
            stage('Build') {
                steps {
                    sh 'scp -i ${keyfile} Cargo.toml localhost:/home/elarnon/experiments/'
                }
            }
        }
    }
}

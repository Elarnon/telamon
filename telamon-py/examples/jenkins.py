import argparse
import json
import pathlib
import toml

from jenkinsapi.jenkins import Jenkins


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('job_name')
    parser.add_argument('--kernel', choices=('matmul', 'matmul-m-32-4-n-32-4-k-32'), default='matmul')
    parser.add_argument('--device', choices=('cpu', 'gpu'), default='gpu')
    parser.add_argument('--max-evaluations', type=int, default=100000)
    parser.add_argument('--threshold', type=int, default=10)
    parser.add_argument('--credentials', default='~/.jenkins')

    ns = parser.parse_args()

    config = {
        'max_evaluations': ns.max_evaluations,
        'algorithm': {
            'type': 'bandit',
            'threshold': ns.threshold,
        },
    }

    with pathlib.Path(ns.credentials).expanduser().open() as f:
        Jenkins(**json.load(f)).build_job(ns.job_name, {
            'settings': toml.dumps(config),
            'kernel': ns.kernel,
            'device': ns.device,
        })

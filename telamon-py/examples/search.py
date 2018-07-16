import os
import telamon as tl
import toml

if __name__ == '__main__':
    if os.path.exists('Settings.toml'):
        with open('Settings.toml') as f:
            config = toml.load(f)
    else:
        config = {'max_evaluations': 4}

    with tl.device('GPU'):
        tl.MatMul(1024, 1024, 1024).optimize(config=config)

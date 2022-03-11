from pyteal import *

"""Always succeeds"""

def program():
    return compileTeal(Int(1), Mode.Signature, version=5)

path = 'teal/always_succeeds.teal'
with open(path, 'w') as f:
    output = program()
    # print(output)
    f.write(output)
    print("Done! Wrote always succeds TEAL to: " + path)

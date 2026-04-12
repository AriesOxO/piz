param(
    [string]$Tape = "assets/tapes/demo-hero.tape",
    [switch]$BuildImage
)

$image = "piz-vhs:local"

if ($BuildImage) {
    docker build -f assets/tapes/Dockerfile.vhs -t $image .
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

$repo = (Resolve-Path ".").Path
$tapeInContainer = "/work/" + ($Tape -replace "\\", "/")

$cmd = @"
set -e
python3 /work/assets/tapes/mock-openai.py &
MOCK_PID=$!
trap 'kill $MOCK_PID' EXIT
/work/assets/tapes/setup-demo-env.sh /work/assets/tapes/.demo-home
CARGO_TARGET_DIR=/work/target-vhs cargo build
vhs $tapeInContainer
"@

docker run --rm -v "${repo}:/work" -w /work $image bash -lc $cmd

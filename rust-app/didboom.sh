cargo build --release
MAXSTACK=`cargo stack-sizes --bin provenance | grep Some | cut -f3 | sed 's/Some(//; s/)//' | sort -n | tail -n1`
GLOBALS=$(printf "%d" 0x`nm target/thumb*/release/provenance | grep canary | cut -f1 -d' ' | tail -c+2`)

echo stack:$MAXSTACK globals:$GLOBALS both:$(($MAXSTACK+$GLOBALS))

#!/usr/bin/env bash
set -e
SERVER=aws-vpn
DEST=/opt/fairy-tales

cargo zigbuild --release --target x86_64-unknown-linux-musl

rsync -azh --info=progress2 \
  --include='config.toml' \
  --include='content/' \
  --include='content/**' \
  --include='static/' \
  --include='static/**' \
  --include='templates/' \
  --include='templates/**' \
  --exclude='*' \
  --rsync-path="sudo rsync" ./ $SERVER:$DEST/

rsync -azh --info=progress2 \
  --rsync-path="sudo rsync" \
  target/x86_64-unknown-linux-musl/release/fairy-tales $SERVER:$DEST/fairy-tales

ssh $SERVER "sudo chown -R fairy-tales:fairy-tales $DEST \
  && sudo chmod +x $DEST/fairy-tales \
  && sudo systemctl restart fairy-tales \
  && sudo systemctl reload nginx"
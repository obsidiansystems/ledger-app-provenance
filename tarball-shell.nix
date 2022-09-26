let
  ledgerPlatform = import (fetchTarball "https://github.com/obsidiansystems/alamgu/archive/develop.tar.gz") {};
  pkgs = ledgerPlatform.pkgs;
  load-app = import ./.;
in
  pkgs.mkShell {
    buildInputs = [load-app];
  }

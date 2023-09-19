with (import <nixpkgs> {});
let
  elementsd-simplicity = elementsd.overrideAttrs (_: rec {
    version = "unstable-2023-08-24a";
    src = fetchFromGitHub {
      owner = "ElementsProject";
      repo = "elements";
      rev = "16fd80c3aca58e059268a11a3cba5f3c0d2607a2"; # <-- update this to latest `simplicity` branch: https://github.com/ElementsProject/elements/commits/simplicity
      sha256 = "sha256-Sv3kMnlnXXmCYshSPwTkgEc/M6aHAQInnqCB7kXW4WY="; # <-- overwrite this, rerun and place the expected hash
    };
  });
in
  mkShell {
    buildInputs = [
      elementsd-simplicity
    ];
  }

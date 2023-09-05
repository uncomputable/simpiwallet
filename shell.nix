with (import <nixpkgs> {});
let
  elementsd-simplicity = elementsd.overrideAttrs (_: rec {
    version = "unstable-2023-08-24";
    src = fetchFromGitHub {
      owner = "ElementsProject";
      repo = "elements";
      rev = "16fd80c3aca58e059268a11a3cba5f3c0d2607a2"; # <-- update this to latest `simplicity` branch: https://github.com/ElementsProject/elements/commits/simplicity
      sha256 = "ooe+If3HWaJWpr2ux7DpiCTqB9Hv+aXjquEjplDjvhM="; # <-- ignore this
    };
  });
in
  mkShell {
    buildInputs = [
      elementsd-simplicity
    ];
  }

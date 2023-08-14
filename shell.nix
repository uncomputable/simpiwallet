with (import <nixpkgs> {});
let
  elementsd-simplicity = elementsd.overrideAttrs (_: rec {
    version = "unstable-2023-06-15";
    src = fetchFromGitHub {
      owner = "ElementsProject";
      repo = "elements";
      rev = "80c5d581ff2a5c9d0d2f759f84575a9fe8204efa"; # <-- update this to latest `simplicity` branch: https://github.com/ElementsProject/elements/commits/simplicity
      sha256 = "ooe+If3HWaJWpr2ux7DpiCTqB9Hv+aXjquEjplDjvhM="; # <-- ignore this
    };
  });
in
  mkShell {
    buildInputs = [
      elementsd-simplicity
    ];
  }

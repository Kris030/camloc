# camloc-aruco

## docker for cross-compilation

#### prerequisites

[QEMU user static](https://github.com/multiarch/qemu-user-static)

```sh
docker run --rm --privileged multiarch/qemu-user-static --reset -p yes
```

#### building

```sh
docker buildx build -t camloc-build .
```

#### running

```sh
# (in project root)
docker run --rm -it --platform linux/arm/v7 -v $PWD:/app camloc-build
```

## issues

The `objdetect` module of opencv does not work on `linux/arm32v7` architectures (compiled from both docker and raspberry).

This issue may be relevant, however the root of the problem might be windows (opencv installed from scoop doesn't ship like half of the modules):
https://github.com/twistedfall/opencv-rust/issues/447

Compiler dump:

```sh
error[E0433]: failed to resolve: could not find `ArucoDetector` in `objdetect`
  --> src/aruco.rs:17:34
   |
17 |             detector: objdetect::ArucoDetector::new(
   |                                  ^^^^^^^^^^^^^ could not find `ArucoDetector` in `objdetect`

error[E0433]: failed to resolve: could not find `PredefinedDictionaryType` in `objdetect`
  --> src/aruco.rs:19:32
   |
19 |                     objdetect::PredefinedDictionaryType::DICT_4X4_50,
   |                                ^^^^^^^^^^^^^^^^^^^^^^^^ could not find `PredefinedDictionaryType` in `objdetect`

error[E0412]: cannot find type `ArucoDetector` in module `objdetect`
 --> src/aruco.rs:5:26
  |
5 |     detector: objdetect::ArucoDetector,
  |                          ^^^^^^^^^^^^^ not found in `objdetect`

error[E0425]: cannot find function `get_predefined_dictionary` in module `objdetect`
  --> src/aruco.rs:18:29
   |
18 |                 &objdetect::get_predefined_dictionary(
   |                             ^^^^^^^^^^^^^^^^^^^^^^^^^ not found in `objdetect`
   |
help: consider importing this function
   |
1  | use opencv::aruco::get_predefined_dictionary;
   |
help: if you import `get_predefined_dictionary`, refer to it directly
   |
18 -                 &objdetect::get_predefined_dictionary(
18 +                 &get_predefined_dictionary(
   |

error[E0433]: failed to resolve: could not find `DetectorParameters` in `objdetect`
  --> src/aruco.rs:21:29
   |
21 |                 &objdetect::DetectorParameters::default()?,
   |                             ^^^^^^^^^^^^^^^^^^ could not find `DetectorParameters` in `objdetect`
   |
help: a trait with a similar name exists
   |
21 |                 &objdetect::DetectorParametersTrait::default()?,
   |                             ~~~~~~~~~~~~~~~~~~~~~~~
help: consider importing this struct
   |
1  | use opencv::aruco::DetectorParameters;
   |
help: if you import `DetectorParameters`, refer to it directly
   |
21 -                 &objdetect::DetectorParameters::default()?,
21 +                 &DetectorParameters::default()?,
   |

error[E0422]: cannot find struct, variant or union type `RefineParameters` in module `objdetect`
  --> src/aruco.rs:22:28
   |
22 |                 objdetect::RefineParameters {
   |                            ^^^^^^^^^^^^^^^^ not found in `objdetect`

```

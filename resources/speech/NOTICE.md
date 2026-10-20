# NVIDIA Parakeet TDT-CTC 110M

Model by NVIDIA NeMo and Suno.ai, https://huggingface.co/nvidia/parakeet-tdt_ctc-110m .
Licensed under Creative Commons Attribution 4.0 International: https://creativecommons.org/licenses/by/4.0/ .

Ion converts upstream revision 431a349f3051ab85c22b9b7a2741b5fe77065665 to GGUF with NVIDIA NeMo-Speech.cpp converter revision caa3f47c7c75fa2e9e6340272ebad57c1647dd44, explicitly selecting TDT and portable Q8_0 block layout. These are format conversion and quantization changes; the weights are not fine-tuned. The artifact includes the auxiliary CTC tensors but inference uses TDT.

The native runtime is NVIDIA NeMo-Speech.cpp, https://github.com/NVIDIA/NeMo-Speech.cpp , under Apache-2.0. Its bundled license/third-party notices remain included in the installed runtime. The manifest pins the converted artifact bytes and SHA-256.

The manifest download URL is the intended release location, not proof it is published. Release requires uploading the exact artifact to that HTTPS URL and verifying it before distribution.

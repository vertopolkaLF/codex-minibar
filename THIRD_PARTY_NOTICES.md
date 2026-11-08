# Third-party notices

The Antigravity and Grok provider mark geometry in `assets/icons/antigravity.svg`
and `assets/icons/grok.svg` was adapted from CodexBar at commit
`28424cbd3069bec62243e762bf9034ac75f5b74b`. The Command Code mark in
`assets/icons/commandcode.svg` was adapted from CodexBar's
`Sources/CodexBar/Resources/ProviderIcon-commandcode.svg`.

CodexBar is Copyright (c) 2025 Peter Steinberger and is distributed under the
MIT License. Its full license is available at:
https://github.com/steipete/CodexBar/blob/main/LICENSE

## AppImage libraries

The AppImage bundles the shared libraries the program links, plus a software
Vulkan renderer in `usr/lib/lavapipe` for hosts without a usable GPU driver:
Mesa's lavapipe (MIT License, https://docs.mesa3d.org/license.html), the Khronos
Vulkan loader (Apache License 2.0, https://github.com/KhronosGroup/Vulkan-Loader)
and the libraries they link, such as LLVM (Apache License 2.0 with LLVM
Exceptions, https://llvm.org/LICENSE.txt). They are copied unmodified from the
Ubuntu packages of the build machine; their licenses and sources are available
from those packages (https://packages.ubuntu.com).

# Manual Installation Guide

Welcome! If the quick-install script didn't work for you, or if you prefer to build the application from scratch, this guide will walk you through the manual installation process step-by-step. 

**You do not need to be a developer to follow these steps.** Just open your Mac's Terminal and follow along.

---

## Step 1: Install Prerequisites

Before downloading ClipWallet, your Mac needs two tools:
1. **Git:** To download the code.
2. **Rust:** The programming language used to build ClipWallet.

Open your **Terminal** (you can open this by pressing `Cmd + Space`, typing "Terminal", and pressing Enter) and paste the following commands one by one, pressing Enter after each:

1. **Install Git:**
   ```sh
   git --version

<img width="992" height="176" alt="image" src="https://github.com/user-attachments/assets/b186d76f-f2b2-4183-a92a-ce7d128ecf12" />

(If Git isn't installed, a prompt will appear asking you to install the Xcode Command Line Tools. Agree and let it install).

    Install Rust:
    Bash

    curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh

    (Press 1 and Enter when prompted to proceed with the standard installation).
    
   <img width="1280" height="906" alt="image" src="https://github.com/user-attachments/assets/8ea7c5db-a952-454e-8ae8-3fa0bb11da9b" />


Step 2: Download ClipWallet

Now that you have the required tools, you can download the code.

    In your terminal, copy and paste the following command to download the repository:
    Bash

    git clone [https://github.com/shaaravraghu/ClipWallet.git](https://github.com/shaaravraghu/ClipWallet.git)

    Move into the newly downloaded folder by running:
    Bash

<img width="1280" height="435" alt="image" src="https://github.com/user-attachments/assets/db8a93e5-e394-49b3-af61-9b1438881ecd" />

    cd ClipWallet
    

Step 3: Build and Run the Application

With the code downloaded, you can now build the application.

    While inside the ClipWallet folder in your terminal, run:
    Bash

    cargo build --release

    (This process might take a few minutes as it downloads and compiles the necessary components. Don't worry if you see a lot of text scrolling by!)

    Once the build is complete, you can find the ready-to-use application inside the target/release/ folder.

    To run the application immediately from the terminal, use:
    Bash

    cargo run --release

Congratulations! You have successfully installed and run ClipWallet manually.

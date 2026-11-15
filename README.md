# Modded Among Us

A small Windows app that installs the Town of Us Mira mod and starts Among Us for you.

It installs Town of Us Mira (from AU-Avengers/TOU-Mira) exactly as the mod makers ship it, with MiraAPI, Reactor and BepInEx. Nothing else is added.

## What you need

- Windows 10 or 11
- Among Us, bought and installed from Steam or Epic Games

## How to use

1. Download `ModdedAmongUs.exe` from the Releases page.
2. Open it.
3. Pick a version and press Play.

The app only starts the game when you press Play. It remembers your last choice.

### Options in the window

- **Load mods**: uncheck it to play normal Among Us. The mod is moved out of the game, not deleted.
- **Version**: pick "Latest" to always get the newest mod, or pick one version to stay on it. Versions marked "(downloaded)" are saved on your PC and start right away.
- **Play**: sets up the version you picked and starts the game.

If you have Among Us in both Steam and Epic Games, pick one in the window and press Play.

## What the app does

1. Finds Among Us in your Steam libraries or in the Epic Games Launcher.
2. Checks if your game is 32-bit (x86) or 64-bit (x64).
3. Lists every Town of Us Mira version made for your store and your game type.
4. Moves the mod you used last into a saved folder, then puts the version you picked into your Among Us folder. It downloads it only if it is not saved yet.
5. Starts the game through Steam or Epic Games, so your login and your copy of the game are used.

If you have no internet, you can still play any version marked "(downloaded)".

## Good to know

- The active mod is in your real Among Us folder. Steam and Epic will start the modded game, even when you start it from the store. Uncheck "Load mods" and press Play once to go back to normal.
- Saved versions are kept in the `ModdedAmongUs` folder inside your Among Us folder. Delete a folder in there to free space.
- Each saved version keeps its own mod settings (`BepInEx/config`).
- The app removes the BepInEx loading splash screen that comes with the mod. That splash is an old .NET Framework program and made Windows ask you to install .NET Framework. The mod works the same without it.
- When Among Us gets an update, the mod can stop working until the mod makers release a new version. Open the app again later to get it.
- Close Among Us before you switch versions.

## Problems

- "Among Us was not found": install the game from Steam or Epic Games and start it once.
- "Could not reach GitHub": check your internet. GitHub can also limit how often you can check (60 times per hour).
- "No TOU-Mira release has a ... zip": the mod makers have not made a build for your store or game type yet.
- "SteamworksAuthFail" in the game: make sure Steam is open and online before you press Play. If it still happens, try the newest version or uncheck "Load mods" to see if it also happens without mods.

## Credits

- Town of Us Mira: https://github.com/AU-Avengers/TOU-Mira
- This app is not made by or connected to Innersloth. Among Us is a trademark of Innersloth.

## License

GNU GPL v3. See `LICENSE`.

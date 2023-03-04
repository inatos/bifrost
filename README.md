# Bifrost
This ECS game of "Breakout" serves to explore the efficacy of rollback P2P sessions via a WebRTC context.

![](./assets/pics/game_cap.png)


## Why Rollback?
Rollback is a netcode technique that reduces input lag between players by speculative execution. To gain a better understanding, let's juxtapose the classic "frame-delay" approach with rollback. Frame-delay waits for all inputs to get sent to the player's game instance then executes the frame. In contrast, rollback treats all player inputs as local and "predicts" the remote players' next action based off their previous inputs. Consequently, this significantly cuts down the lag normally felt online, feeling almost as responsive as offline play. In addition, this enables players to play with people across the world, despite the high pings. This sounds almost too good to be true, right? What happens when the predictions don't line up with the actual inputs? Well when they don't match up, it simply rolls back the game to the first incorrect frame. Next, it once again predicts the players' inputs then advances the game to the current frame—which all happens rather seamlessly. 

Games that require precise input, such as fighting or FPS games, greatly benefit from rollback. To illustrate, many modern games implement it:  Street Fighter 6, Tekken 8, Overwatch, heck they even retrofitted it into many old games i.e. Super Smash Bros. Melee.


## Setup
1. Update to Rust `1.67.1+`.
``` 
rustup update
```
2. Navigate to project root and install WASM toolchain:
```
rustup target install wasm32-unknown-unknown
```
3. Build WASM target:
```
cargo build --target wasm32-unknown-unknown
```
4. Install WebRTC server `matchbox_server`:
```
cargo install matchbox_server
```


## Run Game
1. Start WebRTC server for player to connect to:
```
matchbox_server
```
2. The matchbox server is configured to run locally. Launch two browsers, I suggest running Chrome with an incognito window. 
(If you want to connect elsewhere, just modify the connection vars in `netcode.rs`.) 
3. Launch the game:
```
cargo run --release
```
4. In each browser connect to the game by navigating to `http://127.0.0.1:1334/`.
5. Once both browsers are connected, the game will automatically start.
6. Controls are the standard WASD and arrows.


## Future Development
Currently, I'm working on creating an UE5 game that flips the shooter genre on its head, I intend to use rollback for its netcode. Although I haven't decided which rollback framework I'll use (GGRS, GGPO, etc.), this project has certainly helped dispel the sorcery behind this great technology. With that said, expect to see some UE5 projects in the future!


## References
- [Bevy ECS](https://docs.rs/bevy/0.9.1/bevy/index.html)
- [Bevy GGRS](https://github.com/gschup/bevy_ggrs)
- [Matchbox](https://johanhelsing.studio/posts/introducing-matchbox)
- [Learn ECS](https://gist.github.com/LearnCocos2D/77f0ced228292676689f)
- [RustConf 2018 GameDev](https://www.youtube.com/watch?v=aKLntZcp27M)
- [GGPO Rollback](https://www.ggpo.net/)
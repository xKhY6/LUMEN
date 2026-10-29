# LUMEN - La citadelle silencieuse

Un metroidvania 2D natif en Rust : dix niveaux de cinquante salles,
trente especes, dix boss, architectures et flores propres a chaque biome.
Les sept salles historiques, leurs pouvoirs et leur gardien sont conserves.
L'ecran d'accueil propose de jouer ou de continuer la sauvegarde existante.

## Campagne

Depuis le Refuge, activer le portail au sol avec E/Entree pour rejoindre
le Bois des murmures. Chaque zone comporte 50 salles, trois especes et
un boss dans la derniere salle. Les portails de salle sont bidirectionnels.
Le portail vers le niveau suivant apparait seulement apres la victoire
contre le boss ; cette victoire est sauvegardee. Le dixieme boss ouvre
un portail de retour au Refuge. Des sanctuaires jalonnent chaque zone,
dont un juste avant le boss. Les routes au sol ne demandent aucun pouvoir.

| Niveau | Bestiaire | Boss / attaque | Architecture / flore |
| --- | --- | --- | --- |
| Bois des murmures | Ronceur, Luciole, Sylphe | Matriarche / charge | Arbres / fougeres |
| Cryptes de craie | Scarabee, Spectre, Osselet | Fossoyeur / bonds | Cryptes / lichens |
| Fonderies de braise | Scorie, Salamandre, Fumee | Colosse / tir radial | Fours / braises |
| Jardins des marees | Crabe, Meduse, Anguille | Reine abyssale / salve | Anneaux / coraux |
| Cimes de verre | Eclat, Phalene, Givre | Prisme / onde laterale | Fleches / cristaux |
| Ruches d'ambre | Ouvriere, Frelon, Larve | Monarque / embuscade | Alveoles / fleurs |
| Observatoires du vent | Rafale, Corbeau, Automate | Astromancien / spirale | Observatoires / roseaux |
| Marais des spores | Mycose, Spore, Limace | Mycelium / rafale | Champignons geants / mycelium |
| Citadelles carmin | Duelliste, Gargouille, Arbaletrier | Sentinelle / balayage | Remparts / ronces |
| Sanctuaire de l'aube | Rayon, Seraphin, Echo | Eclipse / poursuite | Arches solaires / fleurs solaires |

Les attaques sont annoncees par un cercle rouge avant emission. Les profils
associent poursuite, charge, bond, vol oscillant, recul ou patrouille a des
tirs distincts. Vitesse, cadence et vitalite varient selon l'espece ; les boss
ont des salves plus larges. Les tirs hostiles sont limites a 128 et bloques
par les murs. Les silhouettes utilisent l'atlas existant, teinte et complete
par des ailes, carapaces ou cretes, sans nouveaux fichiers a telecharger.

Dans **ATELIER > Niveaux**, selectionner le biome puis la salle. **Biome et
bestiaire** modifie noms, couleurs, architecture et flore ; **Objets** modifie
especes et profils de combat ; **Portails** modifie les positions et le verrou
du boss. Les changements sont annullables et enregistres dans le pack.
**Tester** utilise ces donnees sans toucher a la sauvegarde personnelle.
La carte affiche les 50 salles du biome courant et l'etat des dix zones.

## Jouer sous Windows

Double-cliquer sur `lancer.cmd`, ou ouvrir ce dossier dans un nouveau
terminal et executer :

```powershell
cargo run --release
```

L'executable compile est `target/release/lumen.exe`. Il peut etre lance
directement. Les graphismes, animations, palettes, police et sons sont
charges depuis le dossier `assets` : le conserver avec le jeu.
Aucun serveur n'est necessaire. Pour distribuer le jeu hors du depot,
copier le dossier `assets` complet a cote de l'executable.

Le dossier historique `Mvania` est conserve pour les sauvegardes, la
configuration et les outils MinGW afin de preserver les installations existantes.

Rust et MinGW ont ete installes dans le profil utilisateur. Rouvrir VS Code
ou le terminal pour prendre en compte le PATH. Le lanceur configure aussi
le PATH temporairement. Les taches VS Code permettent de jouer, compiler
et tester le projet.

Sur une autre machine : installer Rust et les outils de liaison adaptes.
Sous Windows, utiliser la chaine `stable-x86_64-pc-windows-gnu` avec MinGW,
ou la chaine MSVC avec les Visual Studio Build Tools. Sur Linux, les
bibliotheques de developpement X11 et OpenGL peuvent etre necessaires.
L'atelier demande aussi les bibliotheques d'importation Windows `shlwapi`
et `imm32` : utiliser une distribution MinGW complete si l'editeur de liens
signale leur absence.
La prise en charge des manettes avec gilrs demande aussi libudev sous Linux
(par exemple le paquet libudev-dev).
L'audio necessite aussi les bibliotheques ALSA de developpement sous Linux
(par exemple libasound2-dev).

## Commandes

Ces touches sont les valeurs par defaut. Elles peuvent etre modifiees
depuis le menu Touches de l'accueil ou de la pause.

| Action | Clavier / souris |
| --- | --- |
| Marcher | Fleches gauche/droite, A/D ou Q/D |
| Sauter / double saut | Espace, haut, W ou Z |
| Frapper | J, X ou clic gauche |
| Arme suivante | R (gachette droite RT / R2 a la manette) |
| Ruee (avec l'Aile d'echo) | Maj, K ou C |
| Activer sanctuaire / passage / coeur | E ou Entree |
| Carte | Tab |
| Pause / fermer la carte | Echap |
| Plein ecran | F11 |
| Menu debug (pendant une partie) | F3, ou Pause > DEBUG |

Les menus fonctionnent a la souris, avec haut/bas et Entree/Espace au
clavier, ou avec la croix/le stick et le bouton sud a la manette.
Echap ou le bouton est revient en arriere. Nouvelle partie demande
confirmation avant d'effacer une progression existante. Le menu pause
permet aussi de revenir a l'accueil.

## Mode debug

Ouvrir **DEBUG** depuis la pause ou avec **F3** pendant une partie.
Le panneau suspend la simulation ; **Reprendre**, F3 ou Echap revient au jeu.
Il se pilote a la souris et au clavier (Tab pour parcourir les controles).

- **Joueur** : ajouter une vie, soigner, ajouter jusqu'a quatre vies maximales,
	revenir au sanctuaire, invincibilite, sauts illimites, suppression des recharges,
	double saut/ruee, choix de n'importe quel personnage, meme verrouille (les
	verrouilles sont signales), et vitesse de simulation de 0,1 a 3 fois la normale.
- **Armes** : debloquer le catalogue complet, choisir une arme et regler le
	compteur d'ennemis vaincus. Les armes de packs personnalises sont incluses.
- **Niveaux** : rechercher et rejoindre n'importe quelle salle, acceder aux
	entrees des biomes, recharger la salle, retirer ennemis ou projectiles.
	Vider une salle ne donne aucun butin et ne valide pas la victoire d'un boss.
- **Progression** : eclat de vie, carte revelee, sanctuaire de retour,
	victoire et etat individuel des boss/verrous de portails.

**L'ouverture du debug suspend la sauvegarde automatique pour toute la session**,
meme apres fermeture du panneau ou retour a l'accueil. La sauvegarde personnelle
reste intacte sauf avec **Enregistrer la progression...**, puis confirmation.
Ce bouton enregistre les pouvoirs, armes, visites, sanctuaire et boss, pas la
sante courante, les vies supplementaires, l'invincibilite ou la vitesse.
Nouvelle partie ou un redemarrage du jeu retablit le fonctionnement normal.

Lancer avec `--debug` ouvre directement ce menu. Une capture isolee peut etre
realisee avec `--debug --capture target/debug-menu.png --small` ; elle ne lit
ni n'ecrit la sauvegarde personnelle et le bouton d'enregistrement est desactive.

## Configuration des touches

Ouvrir **Touches**, selectionner une action avec la souris ou haut/bas,
puis valider avec Entree, Espace ou le bouton sud de la manette. Appuyer
sur la nouvelle touche clavier. Echap ou le bouton est annule la capture.
Chaque changement remplace les alias clavier de l'action choisie. Les
autres actions, le clic gauche pour frapper et les boutons manette restent
inchanges. **Par defaut** restaure tous les alias d'origine.

Les choix sont enregistres automatiquement dans un fichier JSON externe :
`%LOCALAPPDATA%/Mvania/config.json` sous Windows, ou `~/Mvania/config.json`
si LOCALAPPDATA n'est pas defini. Le fichier est cree au premier lancement
et recharge a chaque demarrage. Il est independant de `save.json` et n'est
pas efface par Nouvelle partie.

Les fleches haut/bas, W/Z/S, Entree/Espace et Echap restent utilisables
dans les menus quelle que soit la configuration. Echap reste egalement
un raccourci de securite pour mettre en pause. Une touche deja attribuee
a une autre action est refusee. Les combinaisons de touches ne sont pas
prises en charge.

Le JSON peut etre edite jeu ferme : les champs `left`, `right`, `jump`,
`attack`, `next_weapon`, `dash`, `interact`, `map`, `pause`, `fullscreen` contiennent des
listes de noms de touches (par exemple `"B"`, `"Espace"`, `"MajG"`, `"F11"`).
Un champ absent garde ses valeurs par defaut. Un fichier invalide conserve
son contenu et le jeu utilise les valeurs par defaut jusqu'a une modification
volontaire des reglages. Si l'ecriture echoue, la modification n'est pas
appliquee et un message apparait dans le menu.

## Armes

L'epee, la dague rapide, la lance a longue portee, le marteau (3 degats),
l'arc (1 degat), l'arbalete (3 degats) et le baton d'etincelles (2 degats)
sont disponibles des le depart. **R** (ou RT/R2 a la manette) parcourt les
armes debloquees ; le HUD affiche l'arme active et le nombre d'armes possedees.
Un changement demande pendant une frappe est applique a sa fin ; changer d'arme
ne raccourcit pas la recharge. Chaque frappe touche une cible une seule fois.

Six armes a feu et six variantes laser se gagnent automatiquement en vainquant des ennemis :

| Arme | Ennemis vaincus | Degats | Tir |
| --- | --- | --- | --- |
| Pistolet leger | 2 | 1 | Semi-automatique |
| Pistolet lourd | 5 | 3 | Semi-automatique |
| Fusil de combat | 8 | 2 | Semi-automatique |
| Fusil de precision | 12 | 5 | Semi-automatique |
| Mitrailleuse legere | 16 | 1 | Automatique |
| Mitrailleuse lourde | 24 | 2 | Automatique |
| Pistolet laser | 28 | 2 | Semi-automatique |
| Pistolet laser lourd | 32 | 4 | Semi-automatique |
| Fusil laser | 38 | 3 | Semi-automatique |
| Fusil laser de precision | 44 | 7 | Semi-automatique |
| Mitrailleuse laser | 52 | 2 | Automatique |
| Mitrailleuse laser lourde | 64 | 3 | Automatique |

Le HUD **BUTIN** suit la progression vers le prochain gain. Une notification
annonce chaque arme gagnee. Les ennemis qui reapparaissent apres un changement
de salle comptent aussi : toutes les armes peuvent etre obtenues dans une partie.
Maintenir J/X, le clic gauche ou le bouton ouest de la manette pour tirer en
continu avec les mitrailleuses ; les autres armes tirent une fois par pression.

Le catalogue `weapons` du manifeste du pack contient le nom, les degats,
la portee et la hauteur en pixels, les durees de frappe/recharge en secondes,
et la couleur RGB. Les anciens packs sans catalogue recoivent les dix-neuf
armes par defaut. Le compteur `defeated_enemies` et le nom `equipped_weapon`
sont sauvegardes avec la progression et conserves apres la mort.
Les anciens fichiers de sauvegarde restent compatibles. Un pack sans l'arme
precedemment equipee selectionne une arme accessible a la place.

Les armes a distance tirent horizontalement dans la direction du personnage,
avec la meme commande de frappe, sans munitions limitees. Chaque projectile
s'arrete au premier ennemi ou mur, ou a sa portee maximale. Les tirs deja
lances conservent leurs degats apres un changement d'arme et disparaissent
au changement de salle. La simulation limite les projectiles a 64.
Le champ facultatif `projectile` contient `speed` (100-1200 px/s),
`range` (64-1200 px) et `radius` (2-12 px). Sans ce champ, l'arme reste en melee.
Les champs facultatifs `automatic` et `unlock_kills` reglent le tir maintenu
et le nombre d'ennemis requis (0-200, 0 = disponible au depart).
Le pack doit conserver au moins une arme disponible au depart.
Les lasers tirent des impulsions lumineuses a 1200 px/s, bloquees par les murs
et le premier ennemi comme les autres projectiles. Le champ facultatif `laser`
(false par defaut) active la trainee lumineuse et son coeur blanc sans modifier
les collisions. Les tirs conservent ce rendu apres un changement d'arme.
Dans **ATELIER > Armes**, ces options sont modifiables ; **Tester** permet
d'essayer toutes les armes, meme verrouillees, sans toucher a la progression.
Cocher **Distance**, puis **Laser** pour creer une arme laser personnalisee.
Essai direct : `cargo run --release -- --editor --editor-tab weapons --editor-weapon 17 --editor-test`.

## Audio

Une ambiance originale de 24 secondes tourne en boucle, avec des nappes
douces et une melodie de cloches. La musique se fait plus discrete en pause
et sur la carte, puis reprend progressivement son volume normal.
Quatorze bruitages couvrent sauts, atterrissages, frappes, impacts, ruee,
degats, mort, collectes, sanctuaires, passages, victoire et navigation.
Les WAV originaux sont distribues dans `assets/audio` et charges une seule
fois au demarrage. Ils peuvent etre remplaces sans recompiler le jeu.
Aucun telechargement n'est necessaire.

Le menu **Audio** de l'accueil ou de la pause propose des volumes separes
pour la musique et les bruitages. Utiliser les boutons -/+, les fleches
gauche/droite ou le stick/la croix de la manette. Un volume de 0 coupe le
canal. Une modification des bruitages joue un son de test.
Les valeurs `music_volume` et `sfx_volume` (entre 0 et 1) sont sauvegardees
automatiquement dans `config.json`. Les anciens fichiers de configuration
restent compatibles. Les captures restent silencieuses.

Diagnostic de lecture audio, sans toucher aux fichiers personnels :
`cargo run --release -- --check-audio`. Il joue les bruitages et quelques
secondes de musique, puis ferme la fenetre.

## Effets visuels

Le profil **cinematic** est actif par defaut, dans le jeu et le test de
l'atelier. Les effets sont adaptes a chaque salle plutot que tous superposes :

- Combat : etincelles avec trainees, impacts etoiles, nombres de degats,
	ondes et distorsion radiale, flashes, secousses et arcs de frappe.
- Mouvement : silhouettes memorisant les positions reelles, lignes de
	vitesse, ombre de contact, poussiere de saut/atterrissage et poses animees.
- Ambiance : parallaxe, rayons lumineux, brume basse, gouttes, braises,
	feuilles et vegetation au vent, ecume et projections des cascades.
- Lumiere : halos additifs, pulsations et scintillement des sources,
	bloom leger sur les pixels lumineux.
- Image : vignette, contraste, grain fixe discret, aberration chromatique,
	flou directionnel pendant les mouvements rapides, ondulation thermique
	de la Forge et aquatique des Sources, desaturation en vitalite critique.
- Transitions : fondus d'entree de salle. Le profil **retro** ajoute
	posterisation et lignes de balayage CRT.

```powershell
cargo run --release -- --fx cinematic
cargo run --release -- --fx retro
cargo run --release -- --fx clean
cargo run --release -- --reduced-fx
cargo run --release -- --editor --fx retro
```

**clean** conserve le rendu de base sans les nouveaux filtres et couches
d'ambiance. **--reduced-fx** conserve la lumiere et le contraste mais coupe
secousses, flash d'ecran, clignotement d'invincibilite, images fantomes,
lignes de vitesse, parallaxe et deformations. Les deux options se combinent.
Elles sont choisies au lancement, sans modifier les fichiers personnels.

Le HUD, les menus et la carte ne sont pas traites par le shader. Les effets
ne changent ni collisions ni progression. Les files sont bornees : 256
particules, 32 ondes, 24 silhouettes, 8 impacts et 32 nombres de degats.
Les effets temporaires expirent et sont purges au changement de salle.
Le temps de simulation fige l'ambiance et les effets en pause ; aucune
texture de post-traitement n'est allouee a chaque image.

Cette palette couvre les effets courants utiles a LUMEN, pas tous les effets
possibles : ceux d'autres genres ou des outils comme un mixeur de shaders
restent hors de cette version. Les shaders et emetteurs appartiennent au
moteur ; les textures, palettes et frames restent dans le pack externe.

## Gameplay moderne

- **Poids des coups** : un coup donne ou recu fige brievement la simulation
	(hit-stop de 0,05 a 0,09 s). Un ennemi ordinaire touche est etourdi 0,24 s,
	repousse a l'oppose du joueur sans etre projete dans le vide, et perd son
	attaque en cours. Les boss ne reculent pas.
- **Saut mural** : avec l'Aile d'echo, appuyer contre un mur en l'air fait
	glisser lentement (vitesse de chute limitee a 120) ; sauter propulse
	loin du mur. Sans le pouvoir, les routes au sol restent inchangees.
- **Boss** : sous la moitie de leur vitalite, ils attaquent 30 % plus souvent,
	se deplacent 20 % plus vite et ajoutent un anneau lent de six tirs dont les
	breches tournent entre deux salves.
- **Difficulte** (ecran **AUDIO ET FX**) : *Calme* ajoute deux coeurs, porte
	l'invulnerabilite apres un coup a 2,0 s et ralentit les attaques ennemies ;
	*Normal* est la reference ; *Epreuve* reduit l'invulnerabilite a 1,0 s et
	accelere les attaques. Le choix est enregistre dans `config.json` et ne
	modifie ni la sauvegarde ni les collisions.
- **Telemetrie locale** : la sauvegarde compte les morts au total et par
	salle (`deaths`, `death_rooms`, plafonnes) pour reperer les salles trop dures.
	Le total s'affiche sur la carte. Rien n'est envoye hors de la machine.

## Rendu 3D et interface

Le module `src/render3d.rs` ajoute de la profondeur au decor sans toucher aux
sprites ni aux collisions : le plan de jeu est le plan z = 0 d'une camera en
perspective, aligne pixel pour pixel sur le monde 2D. L'oeil se decale avec le
joueur, ce qui donne une vraie parallaxe.

- **Mur de fond** quadrille tres loin derriere le decor.
- **Decor flottant** par biome (cristaux, piliers ou pyramides selon
	l'architecture), facettes eclairees, brouillard de profondeur et trois
	nappes de brume entre les plans.
- **Relief** : les plateformes sont extrudees vers le fond ; seuls les flancs
	exposes et tournes vers l'oeil sont traces.
- **Premier plan** : quelques eclats translucides sur les bords de l'ecran.

Le module `src/ui.rs` regroupe le systeme de design de l'interface (panneaux
biseautes, degrades, jauges, logotype). Le HUD presente cartes separees pour les
coeurs, l'Aile d'echo et sa jauge de ruee, l'arme et le butin, et la mini-carte.

Ecran **AUDIO ET FX** (accueil ou pause) : musique, bruitages, profil d'effets
(cinema, retro, simple), mouvement reduit, profondeur 3D et difficulte. Les
changements sont enregistres dans `config.json` (champs `fx`, `reduced_motion`,
`flat`, `difficulty`) et appliques immediatement. Les options de ligne de
commande `--fx`, `--reduced-fx` et `--no-3d` surchargent la configuration au
lancement. Le profil `clean` reste purement 2D.

## Level design de la campagne

- **Courbe de difficulte** : chaque biome est un rang (0 a 9). Les ennemis
	ordinaires gagnent un point de vie tous les trois biomes, leur vitesse
	augmente de 2,5 % par rang et leur delai d'attaque diminue de 2 % par rang ;
	les boss gagnent deux points de vie par rang.
- **Sept gabarits de salle** : classique (sanctuaires), escalier, colonnade de
	piliers suspendus, iles flottantes, passerelle mobile, salle de piques
	pulsants et arene du boss (derniere salle). Chaque biome les utilise tous.
	Le couloir bas (ligne 14) reste libre dans tous les gabarits : la route au
	sol ne demande jamais de pouvoir ; plateformes et pieges sont facultatifs.
- **Plateformes mobiles** : le champ `platforms` d'une salle (`from`, `to`,
	`tiles`, `period`) decrit un va-et-vient horizontal solide qui porte le
	joueur et les ennemis. Le mouvement est deterministe selon le temps de jeu.
	Seul le mouvement horizontal est accepte (`from.y == to.y`) : la
	bibliotheque de collisions ne porte pas verticalement les acteurs.
- **Piques pulsants** : `pulsing: true` fait alterner les piques de la salle
	sur un cycle de 3 s : sortis 1,6 s, rentres 1,0 s, puis 0,4 s d'avertissement
	(piques dores clignotants, sans degats). Hors de ces salles, les piques
	restent toujours sortis.
- **Lisibilite** : une colonne de lumiere et un anneau de points signalent les
	sanctuaires (dores tant qu'ils ne sont pas le point de retour, menthe une
	fois actifs) et les recompenses (Aile d'echo, Eclat de vie). Une pastille
	**E** apparait pres d'un sanctuaire utilisable. La carte de campagne marque
	les sanctuaires et affiche une legende.

Les anciens packs sans ces champs restent valides. Pour regenerer la campagne
avec ces gabarits dans un nouveau pack : `lumen --export-assets <nouveau dossier>`.

## Equilibrage

`assets/balance.json` regroupe les parametres de jeu modifiables sans
recompiler : physique du joueur (`gravity`, `max_fall`, `walk_speed`,
`acceleration`, `jump_speed`, saut mural, `coyote`, `jump_buffer`, ruee),
ennemis (`speed_per_tier`, `pace_per_tier`, seuil et effets de l'enrage des
boss) et les trois niveaux de difficulte (`bonus_hearts`, `invincibility`,
`attack_pace`). Un champ absent garde sa valeur par defaut ; un fichier absent
donne tous les defauts. Les valeurs sont bornees : un nombre hors limites, un
champ inconnu ou un JSON invalide arrete le demarrage avec un message
explicite, plutot que de produire une simulation instable.

Le fichier fourni reproduit exactement les valeurs par defaut (un test le
verifie). Par exemple, augmenter `jump_speed` a 560 rend le saut plus haut ;
la hauteur du saut simple vaut `jump_speed^2 / (2 * gravity)` et doit rester
sous quatre tuiles pour ne pas rendre inutile le double saut. La vitalite des
ennemis (rang du biome) reste definie dans le code.

## Talents

Chaque ennemi vaincu donne 2 eclats, et un boss 10 de plus la premiere fois
(le HUD affiche **ECLATS**). **Pause > TALENTS** les echange contre six
ameliorations durables, enregistrees avec la sauvegarde (`shards`, `talents`) :

| Talent | Cout | Prerequis | Effet |
| --- | --- | --- | --- |
| Vigueur | 10 | - | +1 coeur maximum |
| Allonge | 8 | - | +15 % de portee des armes de melee |
| Reflexes | 10 | - | +0,5 s d'invulnerabilite apres un coup |
| Tranchant | 16 | Allonge | +1 degat pour toutes les armes |
| Souffle | 12 | Reflexes | recharge de la ruee reduite de 25 % |
| Aimant | 12 | Vigueur | +50 % d'eclats gagnes |

Souris, haut/bas et Entree/Espace ou la manette pilotent l'ecran ; un
message indique un achat impossible (eclats insuffisants, prerequis manquant).
Une sauvegarde modifiee a la main est nettoyee au chargement : talents
inconnus, doublons ou prerequis absents sont retires, et les eclats plafonnes
a 9999. Les anciennes sauvegardes demarrent avec 0 eclat et aucun talent.

## Personnages jouables

**Pause > PERSONNAGES** permet de changer de heros a tout moment (souris,
haut/bas et Entree/Espace, ou manette). Un personnage se debloque avec la
progression sauvegardee ; un message l'annonce, et le choix est enregistre
avec la sauvegarde (`character`). Une sauvegarde qui reference un personnage
encore verrouille revient a Lumen. Changer de heros ajuste la vie courante
au nouveau maximum, sans jamais tuer.

| Personnage | Deblocage | Effets |
| --- | --- | --- |
| Lumen | des le depart | equilibre, sans bonus ni malus |
| Ninja (kunoichi) | 30 ennemis vaincus | +12 % vitesse, ruee 30 % plus rapide, -1 coeur |
| Samourai | 3 boss vaincus | +20 % portee et +1 degat en melee, -5 % vitesse |
| Soldat | 80 ennemis vaincus | +2 coeurs, +1 degat a distance, -8 % vitesse |
| Deesse | 6 boss vaincus | +10 % saut, chute 20 % plus lente, +0,4 s d'invulnerabilite |
| Traqueuse | 150 ennemis vaincus | +1 degat en melee, +8 % vitesse, ruee 15 % plus rapide, -10 % portee |

Les bonus se cumulent avec les talents et la difficulte. Le debug (onglet
**Joueur > Personnage**) permet de jouer n'importe quel heros sans le debloquer ;
si la progression est ensuite enregistree depuis le debug, un heros reste
verrouille revient a Lumen au chargement. La capture `--characters` montre
l'ecran, et
`--hero ninja|samurai|soldier|goddess|stalker` capture un heros en jeu.

Le ninja et la deesse ont une silhouette en sablier : epaules et buste larges,
taille nettement plus fine, hanches amples. La Traqueuse, a la silhouette
elancee (chevelure noire, combinaison sombre a plaques argentees et accents
violets, gantelets et bottes a griffes), garde ce galbe avec buste et hanches
plus larges que la taille. Les cinq heros ont leur propre
jeu de dix poses (marche, saut, chute, glissade murale, reception, ruee, frappe).

### Ennemis equivalents

Chaque heros a son equivalent hostile : **kunoichi**, **samourai**, **soldat**
et **deesse dechue**, avec des couleurs plus sombres et des yeux lumineux.
Ils apparaissent dans la campagne a la place d'une creature (environ quatre
ennemis sur neuf, les quatre apparences presentes dans chaque biome ; les boss
restent des creatures) et changent la difficulte sans changer l'attaque du biome :

| Apparence | Vitalite | Effets |
| --- | --- | --- |
| Creature | rang du biome | reference |
| Kunoichi | -1 | 25 % plus rapide |
| Samourai | +1 | - |
| Soldat | - | attaque 15 % plus souvent |
| Deesse dechue | +2 | 10 % plus lente, attaque 10 % plus souvent |

Une salle decrit l'apparence avec le champ `skin` de chaque ennemi (0 creature,
1 kunoichi, 2 samourai, 3 soldat, 4 deesse ; absent = creature). L'atelier
propose une liste **Apparence** pour chaque ennemi de la salle.

## Sprites haute definition et animations

Le joueur et les ennemis sont fournis en **haute definition** : 32x48 pixels
pour le joueur et 32x32 pour les ennemis, soit le double de l'ancienne grille
de 16x24 et 16x16. Le jeu affiche toujours le personnage a la meme taille a
l'ecran, les collisions ne changent pas.

**Aucun sprite n'est dessine dans le code** : tous les sprites sont des fichiers
PNG de `assets/sprites`, listes dans `assets/manifest.json`, et se retouchent dans
l'atelier ou avec n'importe quel editeur d'images.

Le joueur dispose de dix frames : 0 repos, 1-2 marche, 3 apex du saut / coup recu,
4 frappe, 5 ruee, **6 saut montant**, **7 chute**, **8 glissade murale** et
**9 reception**. Les ennemis ont une cinquieme frame **d'annonce d'attaque**
(corps affaisse) affichee pendant le cercle rouge. Un pack plus ancien de six
frames reste valide : les poses etendues retombent automatiquement sur les poses
d'origine. Les frames supplementaires doivent respecter les dimensions du manifeste.

Les cinq personnages (`ninja`, `samurai`, `soldier`, `goddess`, `stalker`, dix frames
32x48) et leurs equivalents ennemis (`enemy_ninja`, `enemy_samurai`,
`enemy_soldier`, `enemy_goddess`, cinq frames 32x32) sont des sprites du
manifeste comme les autres. Un pack sans ces sprites reste jouable : les
personnages et apparences retombent sur Lumen et la creature. Un test verifie
sur les PNG fournis que le ninja, la deesse et leurs equivalents ennemis ont une
silhouette en sablier (buste et hanches plus larges que la taille).

## Tests de regression

`cargo test` execute 100+ tests sans fenetre ni GPU, dont des parties completes
simulees (`src/regression.rs`) :

- chacune des 514 salles est simulee en inactivite puis en marche : position
	finie, joueur reste dans la salle, vie positive ;
- deux parties identiques (memes entrees) produisent exactement le meme etat ;
- un robot traverse les 500 salles de la campagne dans l'ordre par la route au
	sol, en entrant dans chaque portail (les boss sont marques vaincus : leurs
	combats sont couverts par des tests dedies) ;
- chaque biome garde ses plateformes et son accent lisibles sur le fond
	(contraste de luminance minimal) ;
- une ancienne sauvegarde se charge avec tous les nouveaux champs par defaut.

La lisibilite visuelle est verifiee par ces contrastes et par les captures natives
(`--capture target/image.png --room N`, `--title`, `--pause`, `--map`, `--audio-menu`,
`--talents`, `--no-3d`), a comparer a l'oeil apres un changement de rendu.

## Manette

Brancher une manette USB ou l'associer dans les reglages Bluetooth de
Windows avant ou pendant le jeu. La detection est automatique avec gilrs,
sans configuration dans le jeu. L'accueil indique si une manette est
connectee. Une deconnexion de la manette active met le jeu en pause ;
reconnecter la manette puis reprendre, ou utiliser le clavier.

| Action | Bouton (Xbox / PlayStation) |
| --- | --- |
| Marcher | Stick gauche ou croix directionnelle |
| Sauter / valider un menu | A / Croix (sud) |
| Frapper | X / Carre (ouest) |
| Ruee / retour dans un menu | B / Rond (est) |
| Ruee alternative | RB / R1 |
| Activer sanctuaire / passage / coeur | Y / Triangle (nord) |
| Carte | View / Share ou LB / L1 |
| Pause / reprendre | Start / Options |

Le stick a une zone morte pour eviter la derive. Les manettes doivent
etre reconnues par le systeme ; la compatibilite depend du modele et du pilote.
Diagnostic sans charger ni ecrire de sauvegarde :
`cargo run --release -- --check-controller`.

## Parcours

Explorer les galeries a droite du refuge, puis escalader la forge pour
recuperer l'Aile d'echo. Le pouvoir permet un second saut en l'air.
L'Aile offre aussi une ruee horizontale breve qui evite les degats pendant
le mouvement, mais ne traverse pas les murs. Une seule ruee est possible
en l'air ; elle se recharge au sol avec un delai de 0,45 seconde.
Un passage de retour apparait a droite de la forge. De retour au refuge,
le double saut permet d'atteindre le passage haut vers la tour. Traverser
la tour, vaincre le Veilleur et activer le coeur pour terminer.

Le passage haut de la forge mene a la Serre, puis aux Sources. Traverser
les plateformes de la Serre et escalader les Sources pour trouver un
Eclat de vie : il ajoute un sixieme coeur permanent et soigne entierement.
Le sanctuaire des Sources permet d'y reprendre le voyage. Revenir par la
Serre pour retrouver la forge. Cette branche n'est pas obligatoire pour
affronter le Veilleur.

Les sanctuaires soignent et fixent le point de retour. Le pouvoir et la
victoire sont sauvegardes des leur obtention. La mort conserve le pouvoir.
L'Eclat de vie et les salles visitees sont egalement sauvegardes. Les
sauvegardes de la version a cinq salles restent compatibles.
Les ennemis ordinaires reapparaissent en revenant dans une salle.
Sauvegarde Windows : `%LOCALAPPDATA%/Mvania/save.json`.

## Developpement

### Atelier integre

Ouvrir **ATELIER** depuis l'accueil ou la pause, ou lancer directement :

```powershell
cargo run --release -- --editor
cargo run --release -- --assets "C:/MonPack" --editor
```

- **Niveaux** : peindre le sol, gommer, placer ennemis/gardiens, piques,
	sanctuaires et depart de test, ou generer de nouvelles salles connectees.
- **Sprites** : crayon, gomme, remplissage, pipette, transparence, palette,
	frames, lecture d'animation et modification de la cadence/sequence de marche.
	L'import PNG remplace la frame active et exige les dimensions declarees.
- **Audio** : visualiser la waveform, importer un WAV PCM16, choisir la
	selection, le gain et les fondus, ecouter la retouche ou la reinitialiser.
- **Armes** : creer avec **+**, dupliquer, renommer ou supprimer une arme,
	regler degats, portee, hauteur, duree, recharge et couleur, avec un apercu.
	Le catalogue accepte 1 a 32 armes de noms distincts. **Tester** equipe
	l'arme selectionnee et utilise les reglages non enregistres.

Dans **Armes**, cocher **Distance** pour creer une arme a projectiles,
puis regler vitesse, portee et rayon. L'apercu anime montre le tir.
Ces reglages font partie de la sauvegarde et de l'historique du catalogue.

Acces direct aux armes : `cargo run --release -- --editor --editor-tab weapons`.
Ajouter `--editor-weapon 4` preselectionne l'arc du pack fourni
(indices a partir de 0) ; `--editor-test` lance son essai immediatement.

Le clic gauche applique l'outil ; le clic droit gomme sur les deux canevas.
Ctrl+Z/Ctrl+Y annulent/retablissent niveaux, sprites et armes. Ctrl+S ou
**Enregistrer** sauvegarde l'onglet actif. Conserver une copie du pack avant
une retouche importante : l'enregistrement remplace les assets, pas une copie.
Chaque fichier est remplace via un fichier temporaire ; l'ensemble du pack
n'est pas une transaction unique. Un echec conserve les changements restant
a enregistrer et affiche l'erreur.

**Tester** lance la salle, les pixels et les armes non enregistres dans un bac a sable,
avec le double saut disponible. Echap ou **Retour a l'edition** revient
au document. Le test ne modifie pas la sauvegarde personnelle. Il utilise
le clavier ; l'audio se previsualise dans son onglet. Au retour au jeu,
le pack enregistre est recharge. Quitter avec des changements demande
d'enregistrer tous les onglets, de les abandonner ou d'annuler.

Dans **Niveaux**, choisir la salle de rattachement puis ouvrir **Generation
procedurale**. Regler la **Graine** et la **Difficulte**, puis utiliser
**Generer** ou **Generer et jouer** pour lancer immediatement la nouvelle
salle sans enregistrer. La graine reproduit les memes plateformes ; la
difficulte regle le nombre d'ennemis. Le sol continu, le sanctuaire et les
entrees degagees permettent de jouer sans double saut.

Un portail aller-retour relie automatiquement la salle choisie et la nouvelle
salle : l'activer avec la touche d'interaction (E/Entree par defaut).
L'entree est degagee localement dans la salle de rattachement. Une salle
peut recevoir une seule extension ; choisir ensuite la nouvelle salle pour
prolonger la branche, ou une autre salle pour creer une autre branche.
Le pack accepte jusqu'a 600 salles, dont les 500 salles de campagne. Ctrl+Z/Ctrl+Y
annulent/retablissent ensemble la creation, le passage et son degagement.
**Enregistrer** conserve les salles et leurs connexions pour le jeu normal.

Les sept salles initiales, leurs indices, leurs passages et les objets de
progression du moteur sont conserves. Les extensions heritent du biome de
leur salle source, ou des themes historiques. L'atelier ne cree pas de nouvelles frames et ne propose
pas de pistes/mixage audio multipiste.

### Ressources externes

`assets/manifest.json` est le contrat versionne du pack. Les donnees sont
separees du rendu et de la lecture audio dans `src/content.rs`. Le jeu
charge exclusivement les fichiers du pack, sans regeneration ni repli
silencieux si un asset est absent ou invalide. Les changements enregistres
dans l'atelier sont recharges au retour au jeu ; les modifications externes
necessitent un nouveau lancement.

Le pack contient :

- `sprites/*.png` : images RGBA et frames independantes, sans lissage.
- `audio/*.wav` : musique et bruitages PCM signe 16 bits, mono ou stereo,
	8 a 96 kHz, jusqu'a 300 secondes par fichier.
- `Silkscreen-Regular.ttf` et `OFL.txt` : police et licence conservee.
- `manifest.json` : version 1, references aux fichiers et palettes RGB.
- `levels.json` : version 1, 7 a 600 salles avec grille de collision, ennemis,
  dangers, sanctuaire, depart de test et liaisons `portals` des extensions.
  Les indices des salles restent stables.

Tous les chemins du manifeste utilisent `/`, sont relatifs au dossier du
pack et ne peuvent pas contenir `..`. Les PNG doivent correspondre a la
taille `size` declaree, entre 1 et 2048 pixels par dimension. Chaque
sprite declare `frames`, `fps` et `walk` (indices de frames en boucle).
La cadence doit etre comprise entre 0,1 et 120 frames par seconde.

Les roles graphiques obligatoires sont `player`, `enemy`, `heart`, `fern`,
`crystal`, `glow` et `tile_0` a `tile_6`. Les slots du joueur sont :
0 repos, 1/2 marche, 3 saut/impact, 4 frappe, 5 ruee. Pour les ennemis :
0 repos, 1/2 marche, 3 impact. Modifier les PNG permet de redessiner chaque
pose ; `walk` et `fps` pilotent la sequence de marche. Le rendu conserve
sa taille logique et ses hitboxes, meme si la resolution des PNG change.

`rooms` contient sept palettes, dans l'ordre des salles : `background`,
`stone`, `accent`, chacun sous forme `[rouge, vert, bleu]` entre 0 et 255.
`music` reference le WAV boucle. `effects` associe les identifiants
`jump`, `double_jump`, `land`, `attack`, `dash`, `hurt`, `death`, `hit`,
`defeat`, `pickup`, `shrine`, `portal`, `victory`, `menu` a leurs WAV.
La musique de remplacement doit etre preparee pour boucler sans clic.

Exemple de declaration d'une animation :

```json
"player": {
	"size": [16, 24],
	"frames": ["sprites/player_0.png", "sprites/player_1.png",
						 "sprites/player_2.png", "sprites/player_3.png",
						 "sprites/player_4.png", "sprites/player_5.png"],
	"fps": 10.0,
	"walk": [1, 0, 2, 0]
}
```

Commandes pour travailler sur un pack :

```powershell
cargo run -- --check-assets
cargo run -- --assets "C:/MonPack" --check-assets
cargo run -- --assets "C:/MonPack"
cargo run -- --export-assets target/pack-original
cargo run -- --save-campaign
```

La verification decode tous les PNG et WAV, valide les dimensions,
references, animations et la police. L'export reconstruit les ressources
originales dans un **nouveau** dossier et refuse d'ecraser un pack existant.
Ces deux commandes ne lisent ni n'ecrivent les fichiers personnels.
`Manifest::validate` et `Manifest::save` sont reutilisables par l'editeur ;
l'enregistrement refuse un manifeste invalide avant tout remplacement.

La recherche utilise `--assets` en priorite, puis `assets` a cote de
l'executable, dans le repertoire courant ou a la racine du depot Rust.
Un pack trouve mais invalide est signale explicitement, jamais remplace
par un autre. Les erreurs incluent la reference ou le fichier concerne.

Les grilles utilisent 17 lignes de 30 caracteres : `#` solide, `.` vide.
Les positions sont en pixels dans le monde de 960 x 544. Les ennemis
contiennent `position` et `boss`, les dangers `[x, y, largeur, hauteur]`,
le sanctuaire `shrine` est une position ou `null`, et `spawn` le depart du test.
Les sanctuaires des salles 0, 2 et 6 sont obligatoires. Un ancien pack sans
`levels.json` reprend les salles originales et la campagne ; un fichier present mais
invalide est refuse. Le champ optionnel `portals` contient les liaisons
bidirectionnelles des extensions : `from` et `to` sont les indices de salles,
`entrance` et `exit` leurs positions de depart/arrivee `[x, y]`. Chaque salle
ajoutee a une liaison vers une salle precedente. Les anciens packs sans
`portals` restent compatibles. Les decors proceduraux, shaders, passages
originaux et logique de progression restent dans le moteur.

Le champ facultatif `biomes` contient jusqu'a dix definitions : `name`, `boss`,
`bestiary` (trois noms), `background`, `stone`, `accent` (RGB), `flora` et
`architecture` (0-9). Une salle reference son biome par `biome` (indice ou null).
Chaque ennemi ajoute `species` (0-29) et `behavior` : `legacy`, `charge`,
`leap`, `orbit`, `volley`, `wave`, `ambush`, `spiral`, `burst`, `sweep`, `hunt`.
Un portail ajoute `requires_boss` (indice de sa salle source ou null).
La sauvegarde ajoute `defeated_bosses`, liste des indices de salles vaincues.
Les anciens packs sans biomes recoivent la campagne en memoire, sans ecraser
leurs salles. **Enregistrer** ou `--save-campaign` materialise cette migration
dans le pack courant, sans modifier les fichiers personnels. Un pack trop
grand pour ajouter 500 salles est refuse explicitement. La limite du JSON
de niveaux est de 8 Mio ; les nouveaux champs ont des valeurs compatibles
avec l'ancien format.

### Verification

```powershell
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo build --release
cargo run -- --capture target/refuge.png
cargo run -- --small --capture target/forge.png --room 2
cargo run -- --capture target/serre.png --room 5 --dash
cargo run -- --small --capture target/sources.png --room 6
cargo run -- --capture target/carte.png --room 6 --map
cargo run -- --small --capture target/accueil.png --title
cargo run -- --capture target/effets.png --effects
cargo run -- --small --room 2 --effects --capture target/vfx-forge.png
cargo run -- --room 5 --dash --fx retro --capture target/vfx-retro.png
cargo run -- --room 6 --reduced-fx --capture target/vfx-confort.png
cargo run -- --editor --editor-test --capture target/vfx-atelier.png
cargo run -- --editor --editor-generate 42 --capture target/generation.png
cargo run -- --small --editor --editor-generate 42 --editor-test --capture target/generation-jeu.png
cargo run -- --small --capture target/touches.png --settings
cargo run -- --small --capture target/audio.png --audio-menu
```

Le mode capture simule un deplacement, exporte une image puis ferme la
fenetre. Il ne charge ni n'ecrit la sauvegarde ou la configuration personnelles.

- `src/game.rs` : physique fixe a 120 Hz, salles, combat et progression.
- `src/render.rs` : sprites bitmap originaux, decors, HUD et carte.
- `src/main.rs` : fenetre, entrees, menus et sauvegarde JSON.
- `src/controls.rs` : bindings clavier, configuration JSON, manettes et zone morte.
- `src/audio.rs` : synthese WAV originale, banque de bruitages et boucle musicale.
- `src/content.rs` : contrat JSON, validation et chargement des assets externes.
- Macroquad gere le rendu ; macroquad-platformer gere les collisions.
- La police Silkscreen est distribuee sous SIL OFL dans `assets/OFL.txt`.

Les effets incluent des ondes de saut/impact, des particules, un arc de
frappe, une trainee de ruee, un flash de degats bref et des fondus de salle.
Le personnage anime ses jambes et sa cape en marchant, respire au repos,
se replie en l'air et change de posture pendant une frappe, une ruee ou
un impact. Les ennemis ont un cycle de marche, une reaction aux coups et
le Veilleur se baisse avant sa charge. Ces animations sont purement
visuelles : les collisions et les timings de combat ne changent pas.
L'eclairage ajoute des halos doux autour du personnage, des sanctuaires,
des portails et des pouvoirs. La forge diffuse une lumiere chaude animee ;
les cristaux des Sources emettent une lueur froide. Les frappes, les ondes
et la ruee renforcent brievement l'eclairage local. Le personnage de
l'accueil possede aussi un halo. Le melange additif conserve la lisibilite
de la scene sans modifier le HUD, les collisions ou les sauvegardes.
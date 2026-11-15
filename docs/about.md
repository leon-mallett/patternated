# About

Patternated turns a library of illustrated motifs, one banana, one dinosaur,
one cherry, into repeating patterns that can be printed onto fabric.

## The problem it solves

A textile print is not a picture. It is a tile that has to join itself
invisibly on all sides, so that a roll of fabric several metres long shows no
seam where one copy ends and the next begins. Getting that join right by hand is
fiddly, and getting it wrong is expensive: the mistake is usually invisible on
screen and obvious once the fabric arrives.

Most tools approach this by taking a finished square image and trying to repair
its edges. This one never creates an edge to repair. It places individual motifs
on a mathematical grid and draws each one everywhere the grid says it appears,
including the copies that fall partly outside the tile. The join is correct
because it was never broken.

## What that makes possible

Because the pattern is stored as a list of motifs and positions rather than as
a finished image, three things follow that matter commercially:

- **It can be printed at any size.** A web thumbnail and a four-metre run come
  from the same file, because positions are recorded in millimetres rather than
  pixels.
- **It can be recoloured in one operation**, because motifs store colour
  *slots* rather than colours. Textile ranges sell in colourways, so this is
  half of what the tool is for.
- **It can be edited afterwards**, motif by motif. Nothing is baked in.

## Who it is for

Two clothing brands, sharing the engine and nothing visual:

- **Peel Club**, adult apparel with fruit prints, from one illustrator's
  existing artwork. Live today, and the reason the engine is being built.
- **Dreamwoven**, bespoke children's pyjamas, where the motifs are generated
  per order from a child's interests and nobody reviews the result before it is
  printed. A concept, and the harder problem.

It is not a product sold to other designers. The thing being sold is a garment.

## What it is not

Not an image generator. Generative models have no concept of a repeating tile
and cannot control physical scale, so they are used here to draw single objects
and never to lay out a pattern.

Not a general design tool. It does one job.

use strict;
use warnings;

my $TEXTURE = texture_load("textures/checker.png");

sub setup {
    texture_dispose($TEXTURE);
}
